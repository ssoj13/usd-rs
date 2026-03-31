//! Encoder options and supported feature flags.
//! Reference: `_ref/draco/src/draco/compression/config/encoder_options.h`.

use std::ops::{Deref, DerefMut};

use draco_core::core::options::Options;

use crate::compression::config::draco_options::DracoOptions;
use crate::compression::config::encoding_features::{K_EDGEBREAKER, K_PREDICTIVE_EDGEBREAKER};

/// Encoder options base where attributes are identified by a generic key.
#[derive(Clone, Debug)]
pub struct EncoderOptionsBase<AttributeKeyT>
where
    AttributeKeyT: Ord + Clone,
{
    options: DracoOptions<AttributeKeyT>,
    feature_options: Options,
}

impl<AttributeKeyT> Default for EncoderOptionsBase<AttributeKeyT>
where
    AttributeKeyT: Ord + Clone,
{
    fn default() -> Self {
        Self {
            options: DracoOptions::default(),
            feature_options: Options::new(),
        }
    }
}

impl<AttributeKeyT> EncoderOptionsBase<AttributeKeyT>
where
    AttributeKeyT: Ord + Clone,
{
    pub fn create_default_options() -> Self {
        let mut options = Self::default();
        // Assume all supported features enabled by default.
        options.set_supported_feature(K_EDGEBREAKER, true);
        options.set_supported_feature(K_PREDICTIVE_EDGEBREAKER, true);
        options
    }

    pub fn create_empty_options() -> Self {
        Self::default()
    }

    pub fn get_encoding_speed(&self) -> i32 {
        self.get_global_int("encoding_speed", 5)
    }

    pub fn get_decoding_speed(&self) -> i32 {
        self.get_global_int("decoding_speed", 5)
    }

    pub fn get_speed(&self) -> i32 {
        let encoding_speed = self.get_global_int("encoding_speed", -1);
        let decoding_speed = self.get_global_int("decoding_speed", -1);
        let max_speed = encoding_speed.max(decoding_speed);
        if max_speed == -1 {
            return 5;
        }
        max_speed
    }

    pub fn set_speed(&mut self, encoding_speed: i32, decoding_speed: i32) {
        self.set_global_int("encoding_speed", encoding_speed);
        self.set_global_int("decoding_speed", decoding_speed);
    }

    pub fn is_speed_set(&self) -> bool {
        self.is_global_option_set("encoding_speed") || self.is_global_option_set("decoding_speed")
    }

    pub fn set_supported_feature(&mut self, name: &str, supported: bool) {
        self.feature_options.set_bool(name, supported);
    }

    pub fn is_feature_supported(&self, name: &str) -> bool {
        self.feature_options.get_bool(name)
    }

    pub fn set_feature_options(&mut self, options: Options) {
        self.feature_options = options;
    }

    pub fn feature_options(&self) -> &Options {
        &self.feature_options
    }
}

impl<AttributeKeyT> Deref for EncoderOptionsBase<AttributeKeyT>
where
    AttributeKeyT: Ord + Clone,
{
    type Target = DracoOptions<AttributeKeyT>;

    fn deref(&self) -> &Self::Target {
        &self.options
    }
}

impl<AttributeKeyT> DerefMut for EncoderOptionsBase<AttributeKeyT>
where
    AttributeKeyT: Ord + Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.options
    }
}

/// Encoder options keyed by attribute id.
pub type EncoderOptions = EncoderOptionsBase<i32>;
