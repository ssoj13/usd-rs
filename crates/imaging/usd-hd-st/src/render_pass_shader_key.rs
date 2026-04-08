//! HdSt_RenderPassShaderKey - shader key for render pass shaders.
//!
//! Builds the shader configuration for Storm's render pass based on
//! the active AOV bindings. Determines which shader mixins to include
//! for vertex, tessellation, geometry, and fragment shader stages.
//!
//! Port of C++ `HdSt_RenderPassShaderKey`.

use usd_tf::Token;

/// AOV binding for render pass configuration.
#[derive(Clone, Debug)]
pub struct AovBinding {
    /// Name of the AOV (e.g., "color", "depth", "primId")
    pub aov_name: Token,
    /// Render buffer path
    pub render_buffer_path: String,
}

/// Render pass shader key for Storm.
///
/// Determines the shader mixins needed for each shader stage based on
/// which AOVs are bound. This drives the render pass shader compilation.
///
/// Port of C++ `HdSt_RenderPassShaderKey`.
pub struct RenderPassShaderKey {
    /// GLSLFX source file
    pub glslfx: Token,
    /// Vertex shader mixins
    pub vs: Vec<Token>,
    /// Post-tessellation control shader mixins
    pub ptcs: Vec<Token>,
    /// Post-tessellation vertex shader mixins
    pub ptvs: Vec<Token>,
    /// Tessellation control shader mixins
    pub tcs: Vec<Token>,
    /// Tessellation evaluation shader mixins
    pub tes: Vec<Token>,
    /// Geometry shader mixins
    pub gs: Vec<Token>,
    /// Fragment shader mixins
    pub fs: Vec<Token>,
}

impl RenderPassShaderKey {
    /// Create a render pass shader key from AOV bindings.
    ///
    /// Determines which shader mixins are needed based on whether
    /// color, id, and Neye AOVs are bound.
    pub fn new(aov_bindings: &[AovBinding]) -> Self {
        let mut render_color = false;
        let mut render_id = false;
        let mut render_neye = false;

        for binding in aov_bindings {
            let name = binding.aov_name.as_str();
            if !render_color && name == "color" {
                render_color = true;
            }
            if !render_id && aov_has_id_semantic(name) {
                render_id = true;
            }
            if !render_neye && name == "Neye" {
                render_neye = true;
            }
        }

        // Vertex shader: camera + clip planes
        let vs = vec![
            Token::new("RenderPass.Camera"),
            Token::new("RenderPass.ApplyClipPlanes"),
        ];

        // Post-tess control shader: camera only
        let ptcs = vec![Token::new("RenderPass.Camera")];

        // Post-tess vertex shader: camera + clip planes
        let ptvs = vec![
            Token::new("RenderPass.Camera"),
            Token::new("RenderPass.ApplyClipPlanes"),
        ];

        // Tess control shader: camera only
        let tcs = vec![Token::new("RenderPass.Camera")];

        // Tess eval shader: camera + clip planes
        let tes = vec![
            Token::new("RenderPass.Camera"),
            Token::new("RenderPass.ApplyClipPlanes"),
        ];

        // Geometry shader: camera + clip planes
        let gs = vec![
            Token::new("RenderPass.Camera"),
            Token::new("RenderPass.ApplyClipPlanes"),
        ];

        // Fragment shader: camera + camera FS + conditional mixins
        let mut fs = vec![
            Token::new("RenderPass.Camera"),
            Token::new("RenderPass.CameraFS"),
        ];

        if render_color {
            fs.push(Token::new("Selection.DecodeUtils"));
            fs.push(Token::new("Selection.ComputeColor"));
            fs.push(Token::new("RenderPass.ApplyColorOverrides"));
            fs.push(Token::new("RenderPass.RenderColor"));
        } else {
            fs.push(Token::new("RenderPass.NoSelection"));
            fs.push(Token::new("RenderPass.NoColorOverrides"));
            fs.push(Token::new("RenderPass.RenderColorNoOp"));
        }

        if render_id {
            fs.push(Token::new("RenderPass.RenderId"));
        } else {
            fs.push(Token::new("RenderPass.RenderIdNoOp"));
        }

        if render_neye {
            fs.push(Token::new("RenderPass.RenderNeye"));
        } else {
            fs.push(Token::new("RenderPass.RenderNeyeNoOp"));
        }

        fs.push(Token::new("RenderPass.RenderOutput"));

        Self {
            glslfx: Token::new("renderPass.glslfx"),
            vs,
            ptcs,
            ptvs,
            tcs,
            tes,
            gs,
            fs,
        }
    }

    /// Generate the GLSLFX string for this shader key.
    ///
    /// Produces a GLSLFX configuration that imports the necessary shader
    /// files and defines the technique with all shader stages.
    pub fn get_glslfx_string(&self) -> String {
        let mut ss = String::new();
        ss.push_str("-- glslfx version 0.1\n");

        if !self.glslfx.is_empty() {
            ss.push_str(&format!(
                "#import $TOOLS/hdSt/shaders/{}\n",
                self.glslfx.as_str()
            ));
        }
        // Selection shader import
        ss.push_str("#import $TOOLS/hdx/shaders/selection.glslfx\n");

        ss.push_str("-- configuration\n");
        ss.push_str("{\"techniques\": {\"default\": {\n");

        let mut first_stage = true;
        join_tokens_to(&mut ss, "vertexShader", &self.vs, &mut first_stage);
        join_tokens_to(&mut ss, "tessControlShader", &self.tcs, &mut first_stage);
        join_tokens_to(&mut ss, "tessEvalShader", &self.tes, &mut first_stage);
        join_tokens_to(
            &mut ss,
            "postTessControlShader",
            &self.ptcs,
            &mut first_stage,
        );
        join_tokens_to(
            &mut ss,
            "postTessVertexShader",
            &self.ptvs,
            &mut first_stage,
        );
        join_tokens_to(&mut ss, "geometryShader", &self.gs, &mut first_stage);
        join_tokens_to(&mut ss, "fragmentShader", &self.fs, &mut first_stage);

        ss.push_str("}}}\n");
        ss
    }
}

/// Check if an AOV name has id-render semantics.
fn aov_has_id_semantic(name: &str) -> bool {
    name == "primId" || name == "instanceId"
}

/// Join tokens into a GLSLFX stage definition.
fn join_tokens_to(out: &mut String, stage_name: &str, tokens: &[Token], first: &mut bool) {
    if tokens.is_empty() {
        return;
    }

    if !*first {
        out.push_str(",\n");
    }
    *first = false;

    out.push_str(&format!("\"{}\": {{\n  \"source\": [\n", stage_name));

    let mut first_token = true;
    for token in tokens {
        if token.is_empty() {
            continue;
        }
        if !first_token {
            out.push_str(",\n");
        }
        first_token = false;
        out.push_str(&format!("    \"{}\"", token.as_str()));
    }

    out.push_str("\n  ]\n}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_aov_bindings() {
        let key = RenderPassShaderKey::new(&[]);
        assert_eq!(key.glslfx.as_str(), "renderPass.glslfx");
        // No color = NoSelection + NoColorOverrides + RenderColorNoOp
        assert!(key.fs.iter().any(|t| t == "RenderPass.NoSelection"));
        assert!(key.fs.iter().any(|t| t == "RenderPass.RenderIdNoOp"));
    }

    #[test]
    fn test_color_aov() {
        let bindings = vec![AovBinding {
            aov_name: Token::new("color"),
            render_buffer_path: String::new(),
        }];
        let key = RenderPassShaderKey::new(&bindings);
        // Color = Selection + ColorOverrides + RenderColor
        assert!(key.fs.iter().any(|t| t == "Selection.DecodeUtils"));
        assert!(key.fs.iter().any(|t| t == "RenderPass.RenderColor"));
    }

    #[test]
    fn test_id_aov() {
        let bindings = vec![AovBinding {
            aov_name: Token::new("primId"),
            render_buffer_path: String::new(),
        }];
        let key = RenderPassShaderKey::new(&bindings);
        assert!(key.fs.iter().any(|t| t == "RenderPass.RenderId"));
    }

    #[test]
    fn test_neye_aov() {
        let bindings = vec![AovBinding {
            aov_name: Token::new("Neye"),
            render_buffer_path: String::new(),
        }];
        let key = RenderPassShaderKey::new(&bindings);
        assert!(key.fs.iter().any(|t| t == "RenderPass.RenderNeye"));
    }

    #[test]
    fn test_glslfx_string() {
        let key = RenderPassShaderKey::new(&[]);
        let glslfx = key.get_glslfx_string();
        assert!(glslfx.contains("-- glslfx version 0.1"));
        assert!(glslfx.contains("renderPass.glslfx"));
        assert!(glslfx.contains("selection.glslfx"));
        assert!(glslfx.contains("vertexShader"));
        assert!(glslfx.contains("fragmentShader"));
    }

    #[test]
    fn test_vs_mixins() {
        let key = RenderPassShaderKey::new(&[]);
        assert_eq!(key.vs.len(), 2);
        assert_eq!(key.vs[0].as_str(), "RenderPass.Camera");
        assert_eq!(key.vs[1].as_str(), "RenderPass.ApplyClipPlanes");
    }
}
