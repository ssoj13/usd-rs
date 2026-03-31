//! Draco transcoder library (Rust port of `draco/tools/draco_transcoder_lib.*`).
//!
//! What: Defines `DracoTranscoder` and option structs for glTF scene transcoding.
//! Why: Provides the library API behind the Draco transcoder CLI.
//! How: Loads a scene, applies compression settings, and writes glTF/GLB output.
//! Where used: `draco-cli` `transcoder` subcommand and other scene pipelines.

use crate::core::status::{ok_status, Status, StatusCode};
use crate::core::status_or::StatusOr;
use crate::io::gltf_encoder::GltfEncoder;
use crate::io::scene_io::read_scene_from_file;
use crate::scene::Scene;
use crate::scene::SceneUtils;
use draco_core::compression::draco_compression_options::DracoCompressionOptions;

/// Transcoding options for Draco scene compression.
#[derive(Clone, Debug)]
pub struct DracoTranscodingOptions {
    /// Geometry compression settings applied to scene meshes.
    pub geometry: DracoCompressionOptions,
}

impl Default for DracoTranscodingOptions {
    fn default() -> Self {
        Self {
            geometry: DracoCompressionOptions::default(),
        }
    }
}

/// File options for Draco transcoding operations.
#[derive(Clone, Debug, Default)]
pub struct FileOptions {
    pub input_filename: String,
    pub output_filename: String,
    pub output_bin_filename: String,
    pub output_resource_directory: String,
}

/// Transcoder that reads scenes, applies compression options, and writes glTF.
pub struct DracoTranscoder {
    gltf_encoder: GltfEncoder,
    scene: Option<Box<Scene>>,
    transcoding_options: DracoTranscodingOptions,
}

impl DracoTranscoder {
    /// Creates a transcoder with explicit transcoding options.
    pub fn create(options: &DracoTranscodingOptions) -> StatusOr<Box<DracoTranscoder>> {
        let status = options.geometry.check();
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        let dt = DracoTranscoder {
            gltf_encoder: GltfEncoder::new(),
            scene: None,
            transcoding_options: options.clone(),
        };
        StatusOr::new_value(Box::new(dt))
    }

    /// Creates a transcoder using Draco compression options (deprecated parity API).
    pub fn create_from_compression(
        options: &DracoCompressionOptions,
    ) -> StatusOr<Box<DracoTranscoder>> {
        let mut new_options = DracoTranscodingOptions::default();
        new_options.geometry = options.clone();
        Self::create(&new_options)
    }

    /// Transcodes the input file using compression settings from creation.
    pub fn transcode(&mut self, file_options: &FileOptions) -> Status {
        // Mirror C++ flow: read -> compress -> write.
        let status = self.read_scene(file_options);
        if !status.is_ok() {
            return status;
        }
        let status = self.compress_scene();
        if !status.is_ok() {
            return status;
        }
        self.write_scene(file_options)
    }

    fn read_scene(&mut self, file_options: &FileOptions) -> Status {
        if file_options.input_filename.is_empty() {
            return Status::new(StatusCode::DracoError, "Input filename is empty.");
        }
        if file_options.output_filename.is_empty() {
            return Status::new(StatusCode::DracoError, "Output filename is empty.");
        }
        let scene_or = read_scene_from_file(&file_options.input_filename);
        if !scene_or.is_ok() {
            return scene_or.status().clone();
        }
        self.scene = Some(scene_or.into_value());
        ok_status()
    }

    fn write_scene(&mut self, file_options: &FileOptions) -> Status {
        let scene = match self.scene.as_ref() {
            Some(scene) => scene.as_ref(),
            None => return Status::new(StatusCode::DracoError, "Scene is not loaded."),
        };

        if !file_options.output_bin_filename.is_empty()
            && !file_options.output_resource_directory.is_empty()
        {
            return self.gltf_encoder.encode_file_full(
                scene,
                &file_options.output_filename,
                &file_options.output_bin_filename,
                &file_options.output_resource_directory,
            );
        }
        if !file_options.output_bin_filename.is_empty() {
            return self.gltf_encoder.encode_file_with_bin(
                scene,
                &file_options.output_filename,
                &file_options.output_bin_filename,
            );
        }
        self.gltf_encoder
            .encode_file(scene, &file_options.output_filename)
    }

    fn compress_scene(&mut self) -> Status {
        let scene = match self.scene.as_mut() {
            Some(scene) => scene.as_mut(),
            None => return Status::new(StatusCode::DracoError, "Scene is not loaded."),
        };
        SceneUtils::set_draco_compression_options(Some(&self.transcoding_options.geometry), scene);
        ok_status()
    }
}
