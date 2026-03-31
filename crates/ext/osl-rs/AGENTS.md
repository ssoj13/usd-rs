# osl-rs Agent Architecture

## Module Map (58 modules, 65,600 lines)

```
+===========================================================================+
|                            osl-rs                                         |
|                                                                           |
|  +--- COMPILER (frontend) ---+  +--- RUNTIME (execution) ----+           |
|  |                           |  |                             |           |
|  |  preprocess.rs    1,279   |  |  shadingsys.rs     3,373   |           |
|  |  lexer.rs           484   |  |  interp.rs         4,876   |           |
|  |  parser.rs        1,846   |  |  jit.rs            7,621   |           |
|  |  ast.rs             764   |  |  batched_exec.rs   4,275   |           |
|  |  typecheck.rs     1,702   |  |  batched.rs        1,478   |           |
|  |  codegen.rs       1,825   |  |  optimizer.rs      4,761   |           |
|  |  oso.rs             870   |  |  context.rs        1,120   |           |
|  |  oslc.rs            746   |  |  renderer.rs       1,317   |           |
|  |                           |  |  encodedtypes.rs     336   |           |
|  |  Total: ~9,500 lines      |  |  usd_bridge.rs       440   |           |
|  +---------------------------+  |  shaderglobals.rs     471   |           |
|                                 |                             |           |
|                                 |  Total: ~30,000 lines       |           |
|                                 +-----------------------------+           |
|                                                                           |
|  +--- BUILTINS --------+  +--- BSDF/CLOSURES ---+  +--- SUPPORT ------+ |
|  |                      |  |                      |  |                   | |
|  |  noise.rs     1,557  |  |  bsdf.rs       521  |  |  symbol.rs   934  | |
|  |  simplex.rs     921  |  |  bsdf_ext.rs 2,127  |  |  symtab.rs   276  | |
|  |  gabor.rs       432  |  |  closure.rs    413  |  |  typedesc.rs 543  | |
|  |  color.rs     1,385  |  |  closure_ops   771  |  |  typespec.rs 687  | |
|  |  spline.rs      655  |  |  lpe.rs      1,101  |  |  ustring.rs  409  | |
|  |  builtins.rs  1,538  |  |  accum.rs      537  |  |  dual.rs     725  | |
|  |  stdosl.rs      449  |  |                      |  |  dual_vec.rs 951  | |
|  |  opstring.rs    608  |  |  Total: ~5,500       |  |  math.rs   1,019  | |
|  |  matrix_ops     634  |  +----------------------+  |  message.rs  272  | |
|  |  texture.rs     904  |                            |  hashes.rs   517  | |
|  |  texture_vfx    297  |  +--- TOOLS -----------+  |  pointcloud  609  | |
|  |  color_vfx.rs   263  |  |                      |  |  dict.rs     768  | |
|  |                      |  |  oslquery.rs    448  |  |  journal.rs  354  | |
|  |  Total: ~9,600       |  |  oslinfo.rs     211  |  |  strdecls.rs 353  | |
|  +----------------------+  |  capi.rs        513  |  |  fmt.rs      520  | |
|                            |                      |  |                   | |
|                            |  Total: ~1,200       |  |  Total: ~8,400    | |
|                            +----------------------+  +-------------------+ |
+===========================================================================+
```

## Data Flow: .osl to Pixels

```
                    +------------------+
                    |  .osl source     |
                    |  (shader text)   |
                    +--------+---------+
                             |
                    [1. COMPILE]
                             |
         +-------------------+-------------------+
         |                   |                   |
         v                   v                   v
  +------------+    +--------------+    +-------------+
  | preprocess |    |   lexer      |    |   parser    |
  | #define    |--->| 49 keywords  |--->| recursive   |
  | #include   |    | tokens       |    | descent     |
  +------------+    +--------------+    +------+------+
                                               |
                         +---------------------+
                         v
                  +-------------+    +-------------+
                  | typecheck   |--->|  codegen    |
                  | overloads   |    | struct exp  |
                  | printf args |    | opcode low  |
                  +-------------+    +------+------+
                                            |
                                     +------+------+
                                     |  ShaderIR   |
                                     | symbols[]   |
                                     | opcodes[]   |
                                     | args[]      |
                                     +------+------+
                                            |
                               +------------+------------+
                               |                         |
                        [2. OPTIMIZE]              [3. JIT]
                               |                         |
                        +------+------+           +------+------+
                        | optimizer   |           |  Cranelift  |
                        | const fold  |           |  x86-64     |
                        | peephole    |           |  safe fdiv  |
                        | dead code   |           |  10 tramps  |
                        +------+------+           +------+------+
                               |                         |
                               +------------+------------+
                                            |
                                     [4. EXECUTE]
                                            |
                        +-------------------+-------------------+
                        |                   |                   |
                        v                   v                   v
                 +------------+    +--------------+    +-------------+
                 | interp.rs  |    | batched_exec |    | jit fn ptr  |
                 | scalar     |    | N lanes      |    | native      |
                 | 160+ ops   |    | mask stack   |    | cached      |
                 +-----+------+    +------+-------+    +------+------+
                       |                  |                    |
                       +--------+---------+--------------------+
                                |
                         +------+------+
                         |  context    |
                         |  heap/arena |
                         +------+------+
                                |
              +---------+-------+-------+---------+
              |         |               |         |
              v         v               v         v
        +---------+ +--------+   +---------+ +--------+
        |renderer | |closures|   | noise   | | color  |
        |textures | |BSDFs   |   | derivs  | | OCIO   |
        |xforms   | |28 IDs  |   | Dual2   | | 7+more |
        +---------+ +--------+   +---------+ +--------+
                         |
                  +------+------+
                  |   Ci        |  output closure color
                  | (BSDF tree)|
                  +------+------+
                         |
                  [5. INTEGRATE]
                         |
                  +------+------+
                  | Renderer    |  evaluates Ci with lighting
                  | Arnold/etc  |  produces pixel color
                  +-------------+
```

## Opcode Dispatch (Hot Path)

```
  ShaderIR.opcodes[pc]
       |
       v
  match opcode.name {
       |
       +-- "add"/"sub"/"mul"/"div" --> arithmetic (safe_fdiv for div)
       +-- "assign"/"arraycopy"    --> copy values
       +-- "compref"/"compassign"  --> component access (vec.x, mtx[i][j])
       +-- "if"/"for"/"while"     --> control flow (jump to opcode.jumpaddr[])
       +-- "break"/"continue"     --> loop control
       +-- "exit"/"return"        --> shader/function exit
       +-- "closure"              --> allocate_closure_component(id, params)
       +-- "noise"/"cellnoise"    --> noise.rs dispatch (dim + type)
       +-- "texture"              --> renderer.texture() or procedural stub
       +-- "transformc"           --> color.rs or OCIO
       +-- "getmessage"           --> message.rs (3 or 4 arg form)
       +-- "printf"/"warning"     --> renderer.message() or encoded fmt
       +-- "spline"/"splineinverse" --> spline.rs (6 bases)
       +-- "regex_search"         --> opstring.rs custom engine
       +-- "sincos"               --> args[2] in, args[0]/[1] out
       +-- "smoothstep"           --> Dual2 derivative propagation
       +-- 100+ more              --> builtins.rs dispatch table
  }
```

## Type System Hierarchy

```
  TypeDesc (typedesc.rs, repr(C), OIIO-compatible)
  +----------------------------------------+
  | basetype: u8   (FLOAT, INT, STRING, ..) |
  | aggregate: u8  (SCALAR, VEC2/3/4, MTX) |
  | vecsemantics   (COLOR, POINT, VECTOR,  |
  |                 NORMAL, NOSEMANTICS)    |
  | arraylen: i32  (0=not array, -1=unsized)|
  +----------------------------------------+
         |
         v extends
  TypeSpec (typespec.rs)
  +----------------------------------------+
  | desc: TypeDesc                         |
  | struct_id: Option<usize>  (struct ref) |
  | closure: bool             (closure?)   |
  +----------------------------------------+
         |
         v used by
  Symbol (symbol.rs)
  +----------------------------------------+
  | name: UString                          |
  | type_: TypeSpec                        |
  | symtype: SymType                       |
  |   Param | Local | Output | Global     |
  |   Const | Temp                         |
  | dataoffset: i32                        |
  +----------------------------------------+
```

## Thread Safety Model

```
  ShadingSystem (Arc<Mutex<...>>)
       |
       +-- shader groups (read-only after build)
       |
       +-- create_thread_info() --> ThreadInfo (per-thread)
       |
       +-- get_context() --> ShadingContext (per-thread)
       |        |
       |        +-- heap: Vec<f32>         (thread-local)
       |        +-- closure_arena: Vec<u8> (thread-local)
       |        +-- messages: MessageList  (thread-local)
       |        +-- max_warnings: i32
       |
       +-- release_context()  (return to pool)
       +-- destroy_thread_info()

  UString (ustring.rs)
  +-- DashMap with 64 shards (lock-free reads, shard-level writes)
  +-- 16 bytes per UString (ptr + cached hash)

  Batched execution:
  +-- N lanes execute in lockstep
  +-- mask stack tracks active lanes per control flow level
  +-- set_masked() ensures inactive lanes are never written
```
