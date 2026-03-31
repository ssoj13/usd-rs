# osl-rs Architecture Diagrams

## 1. Module Dependency Graph

```mermaid
graph TD
    subgraph Compiler
        PARSER[parser.rs / lexer.rs]
        AST[ast.rs]
        TC[typecheck.rs]
        CG[codegen.rs]
        OSLC[oslc.rs]
        SYMTAB[symtab.rs]
    end

    subgraph Runtime
        SS[shadingsys.rs]
        CTX[context.rs]
        INTERP[interp.rs]
        OSO[oso.rs]
    end

    subgraph Operations
        OPS_STR[opstring.rs]
        OPS_NOISE[noise.rs]
        OPS_SPLINE[spline.rs]
        OPS_COLOR[color.rs]
        OPS_MAT[matrix_ops.rs]
        OPS_CLOS[closure_ops.rs]
        OPS_TEX[texture.rs]
        OPS_MSG[message.rs]
    end

    subgraph Math
        DUAL[dual.rs]
        DUALV[dual_vec.rs]
        HASH[hashes.rs]
        SIMPLEX[simplex.rs]
        GABOR[gabor.rs]
    end

    subgraph Integration
        REND[renderer.rs]
        LPE[lpe.rs]
        ACCUM[accum.rs]
        CLOS[closure.rs]
        DICT[dict.rs]
    end

    OSLC --> PARSER
    PARSER --> AST
    AST --> TC
    TC --> CG
    CG --> SYMTAB
    
    SS --> CTX
    SS --> OSO
    CTX --> INTERP
    CTX --> REND
    
    INTERP --> OPS_STR
    INTERP --> OPS_NOISE
    INTERP --> OPS_SPLINE
    INTERP --> OPS_COLOR
    INTERP --> OPS_MAT
    INTERP --> OPS_CLOS
    INTERP --> OPS_TEX
    INTERP --> OPS_MSG
    INTERP --> DICT
    
    OPS_NOISE --> HASH
    OPS_NOISE --> SIMPLEX
    OPS_NOISE --> GABOR
    OPS_NOISE --> DUAL
    OPS_SPLINE --> DUAL
    OPS_MAT --> DUALV
    OPS_CLOS --> CLOS
    
    LPE --> ACCUM
    REND --> OPS_TEX
```

## 2. OSL Compilation Pipeline

```mermaid
flowchart LR
    SRC[".osl source"] --> LEX["Lexer<br/>lexer.rs"]
    LEX --> TOK["Token Stream"]
    TOK --> PAR["Parser<br/>parser.rs"]
    PAR --> AST["AST<br/>ast.rs"]
    AST --> TC["Type Check<br/>typecheck.rs"]
    TC --> TAST["Typed AST"]
    TAST --> CG["CodeGen<br/>codegen.rs"]
    CG --> OSO[".oso bytecode"]
    OSO --> LOAD["OSO Loader<br/>oso.rs"]
    LOAD --> SM["ShaderMaster"]
    SM --> SI["ShaderInstance"]
    SI --> GRP["ShaderGroup"]
```

## 3. Runtime Execution Flow

```mermaid
sequenceDiagram
    participant R as Renderer
    participant SS as ShadingSystem
    participant CTX as ShadingContext
    participant I as Interpreter
    participant RS as RendererServices

    R->>SS: execute(group, globals)
    SS->>CTX: execute_init(group)
    CTX->>I: new(group)
    
    loop For each layer
        SS->>CTX: execute_layer(layer_idx)
        CTX->>I: run_layer(layer_idx)
        
        loop For each opcode
            I->>I: dispatch(opcode)
            
            alt Texture lookup
                I->>RS: texture(filename, s, t, opts)
                RS-->>I: TextureResult
            else Transform
                I->>RS: get_matrix(from, to)
                RS-->>I: Matrix44
            else Attribute
                I->>RS: get_attribute(name, type)
                RS-->>I: AttributeData
            end
        end
    end
    
    SS->>CTX: execute_cleanup()
    CTX-->>R: ShaderGlobals + Closures
```

## 4. Parity Status Heatmap

```mermaid
graph LR
    subgraph "100% Parity"
        style H fill:#0d0
        H[hashes.rs]
    end
    
    subgraph "95-99% Parity"
        style D fill:#4d4
        style DV fill:#4d4
        style CL fill:#4d4
        style SX fill:#4d4
        D[dual.rs]
        DV[dual_vec.rs]
        CL[closure.rs]
        SX[simplex.rs]
    end
    
    subgraph "85-94% Parity"
        style N fill:#dd0
        style CO fill:#dd0
        style RE fill:#dd0
        style BI fill:#dd0
        style CLO fill:#dd0
        N[noise.rs]
        CO[color.rs]
        RE[renderer.rs]
        BI[builtins.rs]
        CLO[closure_ops.rs]
    end
    
    subgraph "60-84% Parity"
        style SP fill:#da0
        style MA fill:#da0
        style TX fill:#da0
        style MS fill:#da0
        style SH fill:#da0
        SP[spline.rs]
        MA[matrix_ops.rs]
        TX[texture.rs]
        MS[message.rs]
        SH[shadingsys.rs]
    end
    
    subgraph "Below 60% Parity"
        style LP fill:#d00
        style GA fill:#d00
        style DI fill:#d00
        style FM fill:#d00
        LP[lpe.rs]
        GA[gabor.rs]
        DI[dict.rs]
        FM[fmt.rs - DEAD]
    end
```

## 5. Closure Tree Structure

```mermaid
graph TD
    ADD["ClosureAdd"] --> A["closure A"]
    ADD --> B["closure B"]
    
    MUL["ClosureMul<br/>weight: Color3"] --> C["child closure"]
    
    COMP["ClosureComponent<br/>id, weight, params"]
    
    subgraph "Example: glass shader"
        ROOT["Add"] --> REFL["Mul (0.95)"]
        ROOT --> REFR["Mul (0.05)"]
        REFL --> MICROFACET["Component<br/>id=microfacet_ggx"]
        REFR --> REFRACTION["Component<br/>id=refraction"]
    end
```

## 6. Noise Type Dispatch

```mermaid
graph TD
    NOISE["noise_by_name(name)"] -->|"perlin"| PERLIN[perlin3]
    NOISE -->|"uperlin"| UPERLIN[uperlin3]
    NOISE -->|"cell"| CELL[cellnoise3]
    NOISE -->|"hash"| HASH2[hashnoise3]
    NOISE -->|"simplex"| SIMPLEX[simplex3]
    NOISE -->|"usimplex"| USIMPLEX[usimplex3]
    NOISE -->|"gabor"| GABOR[gabor3]
    NOISE -->|"null"| NULL["0.0 (MISSING dispatch)"]
    NOISE -->|"unull"| UNULL["0.5 (MISSING dispatch)"]
    NOISE -->|unknown| DEFAULT["0.0 (no error)"]
    
    PERLIN --> BJH["Bob Jenkins Hash<br/>hashes.rs"]
    CELL --> BJH
    HASH2 --> BJH
    SIMPLEX --> SMOD["simplex.rs"]
    GABOR --> GMOD["gabor.rs"]
```

## 7. Color Space Conversion Graph

```mermaid
graph LR
    RGB["RGB (linear)"]
    HSV["HSV"]
    HSL["HSL"]
    XYZ["CIE XYZ"]
    xyY["CIE xyY"]
    sRGB["sRGB"]
    YIQ["YIQ"]
    BB["Blackbody T(K)"]
    WL["Wavelength (nm)"]
    
    RGB <-->|"rgb_to_hsv / hsv_to_rgb"| HSV
    RGB <-->|"rgb_to_hsl / hsl_to_rgb"| HSL
    RGB <-->|"XYZ_to_RGB / RGB_to_XYZ"| XYZ
    XYZ <-->|"xyz_to_xyy / xyy_to_xyz"| xyY
    RGB <-->|"linear_to_srgb / srgb_to_linear"| sRGB
    RGB <-->|"rgb_to_yiq / yiq_to_rgb"| YIQ
    BB -->|"blackbody_rgb"| RGB
    WL -->|"wavelength_color"| RGB
    
    style HSL fill:#ff9
    style XYZ fill:#ff9
```
