use diffusion_rs::preset::{
    AnimaWeight, ChromaRadianceWeight, ChromaWeight, DiffInstructStarWeight, Flux1MiniWeight,
    Flux1Weight, Flux2Klein4BWeight, Flux2Klein9BWeight, Flux2KleinBase4BWeight,
    Flux2KleinBase9BWeight, Flux2Weight, NitroSDRealismWeight, NitroSDVibrantWeight,
    OvisImageWeight, Preset, QwenImageWeight, SSD1BWeight, TwinFlowZImageTurboExpWeight,
    ZImageTurboWeight,
};

use crate::GenerateError;

macro_rules! parse_preset_weight {
    ($value:expr, $default:expr, { $($pattern:pat => $variant:path),+ $(,)? }) => {{
        let Some(value) = $value else {
            return Ok($default);
        };
        let normalized = normalize_choice(value);
        match normalized.as_str() {
            "auto" | "default" => Ok($default),
            $($pattern => Ok($variant),)+
            _ => Err(GenerateError::InvalidField {
                field: "preset_weight_type",
                message: format!("unknown preset weight type `{value}`"),
            }),
        }
    }};
}

pub(crate) fn parse_preset(
    value: &str,
    weight_type: Option<&str>,
) -> Result<Preset, GenerateError> {
    let normalized = normalize_choice(value);
    if let Some(preset) = static_preset(&normalized) {
        return Ok(preset);
    }
    weighted_preset(&normalized, weight_type)?.ok_or_else(|| GenerateError::InvalidField {
        field: "preset",
        message: format!("unknown preset `{value}`"),
    })
}

fn static_preset(normalized: &str) -> Option<Preset> {
    match normalized {
        "stable_diffusion_1_4" | "sd1_4" => Some(Preset::StableDiffusion1_4),
        "stable_diffusion_1_5" | "sd1_5" => Some(Preset::StableDiffusion1_5),
        "stable_diffusion_2_1" | "sd2_1" => Some(Preset::StableDiffusion2_1),
        "stable_diffusion_3_medium" | "sd3_medium" => Some(Preset::StableDiffusion3Medium),
        "stable_diffusion_3_5_medium" | "sd3_5_medium" => Some(Preset::StableDiffusion3_5Medium),
        "stable_diffusion_3_5_large" | "sd3_5_large" => Some(Preset::StableDiffusion3_5Large),
        "stable_diffusion_3_5_large_turbo" | "sd3_5_large_turbo" => {
            Some(Preset::StableDiffusion3_5LargeTurbo)
        }
        "sdxl_base_1_0" | "sdxl_base" => Some(Preset::SDXLBase1_0),
        "sd_turbo" => Some(Preset::SDTurbo),
        "sdxl_turbo_1_0" | "sdxl_turbo" => Some(Preset::SDXLTurbo1_0),
        "juggernaut_xl_11" | "juggernaut_xl" => Some(Preset::JuggernautXL11),
        "dream_shaper_xl_2_1_turbo" | "dreamshaper_xl_2_1_turbo" => {
            Some(Preset::DreamShaperXL2_1Turbo)
        }
        "sdxs512_dream_shaper" => Some(Preset::SDXS512DreamShaper),
        "segmind_vega" => Some(Preset::SegmindVega),
        _ => None,
    }
}

fn weighted_preset(
    normalized: &str,
    weight_type: Option<&str>,
) -> Result<Option<Preset>, GenerateError> {
    let preset = match normalized {
        "flux_1_dev" | "flux1_dev" => Preset::Flux1Dev(parse_flux1_weight(weight_type)?),
        "flux_1_schnell" | "flux1_schnell" => {
            Preset::Flux1Schnell(parse_flux1_weight(weight_type)?)
        }
        "flux_1_mini" | "flux1_mini" => Preset::Flux1Mini(parse_flux1_mini_weight(weight_type)?),
        "chroma" => Preset::Chroma(parse_chroma_weight(weight_type)?),
        "nitro_sd_realism" => Preset::NitroSDRealism(parse_nitro_weight(weight_type)?),
        "nitro_sd_vibrant" => Preset::NitroSDVibrant(parse_nitro_vibrant_weight(weight_type)?),
        "diff_instruct_star" => {
            Preset::DiffInstructStar(parse_diff_instruct_star_weight(weight_type)?)
        }
        "chroma_radiance" => Preset::ChromaRadiance(parse_chroma_radiance_weight(weight_type)?),
        "ssd_1b" => Preset::SSD1B(parse_ssd1b_weight(weight_type)?),
        "flux_2_dev" | "flux2_dev" => Preset::Flux2Dev(parse_flux2_weight(weight_type)?),
        "z_image_turbo" | "zimage_turbo" => {
            Preset::ZImageTurbo(parse_z_image_turbo_weight(weight_type)?)
        }
        "qwen_image" => Preset::QwenImage(parse_qwen_image_weight(weight_type)?),
        "ovis_image" => Preset::OvisImage(parse_ovis_image_weight(weight_type)?),
        "twinflow_z_image_turbo" | "twinflow_z_image_turbo_exp" => {
            Preset::TwinFlowZImageTurboExp(parse_twinflow_z_image_turbo_weight(weight_type)?)
        }
        "flux_2_klein_4b" | "flux2_klein_4b" => {
            Preset::Flux2Klein4B(parse_flux2_klein_4b_weight(weight_type)?)
        }
        "flux_2_klein_base_4b" | "flux2_klein_base_4b" => {
            Preset::Flux2KleinBase4B(parse_flux2_klein_base_4b_weight(weight_type)?)
        }
        "flux_2_klein_9b" | "flux2_klein_9b" => {
            Preset::Flux2Klein9B(parse_flux2_klein_9b_weight(weight_type)?)
        }
        "flux_2_klein_base_9b" | "flux2_klein_base_9b" => {
            Preset::Flux2KleinBase9B(parse_flux2_klein_base_9b_weight(weight_type)?)
        }
        "anima" => Preset::Anima(parse_anima_weight(weight_type)?),
        _ => return Ok(None),
    };
    Ok(Some(preset))
}

fn parse_flux1_weight(value: Option<&str>) -> Result<Flux1Weight, GenerateError> {
    parse_preset_weight!(value, Flux1Weight::default(), {
        "q4_0" => Flux1Weight::Q4_0,
        "q8_0" => Flux1Weight::Q8_0,
        "q2_k" => Flux1Weight::Q2_K,
        "q3_k" => Flux1Weight::Q3_K,
        "q4_k" => Flux1Weight::Q4_K,
    })
}

fn parse_flux1_mini_weight(value: Option<&str>) -> Result<Flux1MiniWeight, GenerateError> {
    parse_preset_weight!(value, Flux1MiniWeight::default(), {
        "f32" => Flux1MiniWeight::F32,
        "q8_0" => Flux1MiniWeight::Q8_0,
        "q2_k" => Flux1MiniWeight::Q2_K,
        "q3_k" => Flux1MiniWeight::Q3_K,
        "q5_k" => Flux1MiniWeight::Q5_K,
        "q6_k" => Flux1MiniWeight::Q6_K,
        "bf16" => Flux1MiniWeight::BF16,
    })
}

fn parse_chroma_weight(value: Option<&str>) -> Result<ChromaWeight, GenerateError> {
    parse_preset_weight!(value, ChromaWeight::default(), {
        "bf16" => ChromaWeight::BF16,
        "q4_0" => ChromaWeight::Q4_0,
        "q8_0" => ChromaWeight::Q8_0,
    })
}

fn parse_nitro_weight(value: Option<&str>) -> Result<NitroSDRealismWeight, GenerateError> {
    parse_preset_weight!(value, NitroSDRealismWeight::default(), {
        "f16" => NitroSDRealismWeight::F16,
        "q2_k" => NitroSDRealismWeight::Q2_K,
        "q3_k" => NitroSDRealismWeight::Q3_K,
        "q4_0" => NitroSDRealismWeight::Q4_0,
        "q5_0" => NitroSDRealismWeight::Q5_0,
        "q6_k" => NitroSDRealismWeight::Q6_K,
        "q8_0" => NitroSDRealismWeight::Q8_0,
    })
}

fn parse_chroma_radiance_weight(
    value: Option<&str>,
) -> Result<ChromaRadianceWeight, GenerateError> {
    parse_preset_weight!(value, ChromaRadianceWeight::default(), {
        "bf16" => ChromaRadianceWeight::BF16,
        "q8_0" => ChromaRadianceWeight::Q8_0,
    })
}

fn parse_ssd1b_weight(value: Option<&str>) -> Result<SSD1BWeight, GenerateError> {
    parse_preset_weight!(value, SSD1BWeight::default(), {
        "f16" => SSD1BWeight::F16,
        "f8_e4m3" | "fp8_e4m3" | "f8_e4m3fn" | "fp8_e4m3fn" => SSD1BWeight::F8_E4M3,
    })
}

fn parse_flux2_weight(value: Option<&str>) -> Result<Flux2Weight, GenerateError> {
    parse_preset_weight!(value, Flux2Weight::default(), {
        "q4_0" => Flux2Weight::Q4_0,
        "q4_1" => Flux2Weight::Q4_1,
        "q5_0" => Flux2Weight::Q5_0,
        "q5_1" => Flux2Weight::Q5_1,
        "q8_0" => Flux2Weight::Q8_0,
        "q2_k" => Flux2Weight::Q2_K,
        "q3_k" | "q3_k_m" => Flux2Weight::Q3_K,
        "q4_k" | "q4_k_m" => Flux2Weight::Q4_K,
        "q5_k" | "q5_k_m" => Flux2Weight::Q5_K,
        "q6_k" => Flux2Weight::Q6_K,
        "bf16" => Flux2Weight::BF16,
    })
}

fn parse_z_image_turbo_weight(value: Option<&str>) -> Result<ZImageTurboWeight, GenerateError> {
    parse_preset_weight!(value, ZImageTurboWeight::default(), {
        "q4_0" => ZImageTurboWeight::Q4_0,
        "q5_0" => ZImageTurboWeight::Q5_0,
        "q8_0" => ZImageTurboWeight::Q8_0,
        "q2_k" => ZImageTurboWeight::Q2_K,
        "q3_k" | "q3_k_m" => ZImageTurboWeight::Q3_K,
        "q4_k" | "q4_k_m" => ZImageTurboWeight::Q4_K,
        "q6_k" | "q6_0" => ZImageTurboWeight::Q6_K,
        "bf16" => ZImageTurboWeight::BF16,
    })
}

fn parse_qwen_image_weight(value: Option<&str>) -> Result<QwenImageWeight, GenerateError> {
    parse_preset_weight!(value, QwenImageWeight::default(), {
        "q4_0" => QwenImageWeight::Q4_0,
        "q4_1" => QwenImageWeight::Q4_1,
        "q5_0" => QwenImageWeight::Q5_0,
        "q5_1" => QwenImageWeight::Q5_1,
        "q8_0" => QwenImageWeight::Q8_0,
        "q2_k" => QwenImageWeight::Q2_K,
        "q3_k" | "q3_k_m" => QwenImageWeight::Q3_K,
        "q4_k" | "q4_k_m" => QwenImageWeight::Q4_K,
        "q5_k" | "q5_k_m" => QwenImageWeight::Q5_K,
        "q6_k" => QwenImageWeight::Q6_K,
        "bf16" => QwenImageWeight::BF16,
        "f8_e4m3" | "fp8_e4m3" | "f8_e4m3fn" | "fp8_e4m3fn" => QwenImageWeight::F8_E4M3,
    })
}

fn parse_ovis_image_weight(value: Option<&str>) -> Result<OvisImageWeight, GenerateError> {
    parse_preset_weight!(value, OvisImageWeight::default(), {
        "q4_0" => OvisImageWeight::Q4_0,
        "q8_0" => OvisImageWeight::Q8_0,
        "bf16" => OvisImageWeight::BF16,
    })
}

fn parse_twinflow_z_image_turbo_weight(
    value: Option<&str>,
) -> Result<TwinFlowZImageTurboExpWeight, GenerateError> {
    parse_preset_weight!(value, TwinFlowZImageTurboExpWeight::default(), {
        "q4_0" => TwinFlowZImageTurboExpWeight::Q4_0,
        "q5_0" => TwinFlowZImageTurboExpWeight::Q5_0,
        "q8_0" => TwinFlowZImageTurboExpWeight::Q8_0,
        "q3_k" | "q3_k_m" => TwinFlowZImageTurboExpWeight::Q3_K,
        "q6_k" => TwinFlowZImageTurboExpWeight::Q6_K,
        "bf16" => TwinFlowZImageTurboExpWeight::BF16,
    })
}

fn parse_flux2_klein_4b_weight(value: Option<&str>) -> Result<Flux2Klein4BWeight, GenerateError> {
    parse_preset_weight!(value, Flux2Klein4BWeight::default(), {
        "q4_0" => Flux2Klein4BWeight::Q4_0,
        "q8_0" => Flux2Klein4BWeight::Q8_0,
        "bf16" => Flux2Klein4BWeight::BF16,
    })
}

fn parse_flux2_klein_base_4b_weight(
    value: Option<&str>,
) -> Result<Flux2KleinBase4BWeight, GenerateError> {
    parse_preset_weight!(value, Flux2KleinBase4BWeight::default(), {
        "q4_0" => Flux2KleinBase4BWeight::Q4_0,
        "q8_0" => Flux2KleinBase4BWeight::Q8_0,
        "bf16" => Flux2KleinBase4BWeight::BF16,
    })
}

fn parse_flux2_klein_9b_weight(value: Option<&str>) -> Result<Flux2Klein9BWeight, GenerateError> {
    parse_preset_weight!(value, Flux2Klein9BWeight::default(), {
        "q4_0" => Flux2Klein9BWeight::Q4_0,
        "q8_0" => Flux2Klein9BWeight::Q8_0,
        "bf16" => Flux2Klein9BWeight::BF16,
    })
}

fn parse_flux2_klein_base_9b_weight(
    value: Option<&str>,
) -> Result<Flux2KleinBase9BWeight, GenerateError> {
    parse_preset_weight!(value, Flux2KleinBase9BWeight::default(), {
        "q4_0" => Flux2KleinBase9BWeight::Q4_0,
        "bf16" => Flux2KleinBase9BWeight::BF16,
    })
}

fn parse_anima_weight(value: Option<&str>) -> Result<AnimaWeight, GenerateError> {
    parse_preset_weight!(value, AnimaWeight::default(), {
        "q4_k" | "q4_k_m" => AnimaWeight::Q4_K,
        "q5_k" | "q5_k_m" => AnimaWeight::Q5_K,
        "q6_k" => AnimaWeight::Q6_K,
        "bf16" => AnimaWeight::BF16,
        "q4_0" => AnimaWeight::Q4_0,
        "q4_1" => AnimaWeight::Q4_1,
        "q5_0" => AnimaWeight::Q5_0,
        "q5_1" => AnimaWeight::Q5_1,
        "q8_0" => AnimaWeight::Q8_0,
        "q3_k" | "q3_k_l" => AnimaWeight::Q3_K,
    })
}

fn parse_nitro_vibrant_weight(value: Option<&str>) -> Result<NitroSDVibrantWeight, GenerateError> {
    parse_preset_weight!(value, NitroSDVibrantWeight::default(), {
        "f16" => NitroSDVibrantWeight::F16,
        "q2_k" => NitroSDVibrantWeight::Q2_K,
        "q3_k" => NitroSDVibrantWeight::Q3_K,
        "q4_0" => NitroSDVibrantWeight::Q4_0,
        "q5_0" => NitroSDVibrantWeight::Q5_0,
        "q6_k" => NitroSDVibrantWeight::Q6_K,
        "q8_0" => NitroSDVibrantWeight::Q8_0,
    })
}

fn parse_diff_instruct_star_weight(
    value: Option<&str>,
) -> Result<DiffInstructStarWeight, GenerateError> {
    parse_preset_weight!(value, DiffInstructStarWeight::default(), {
        "f16" => DiffInstructStarWeight::F16,
        "q2_k" => DiffInstructStarWeight::Q2_K,
        "q3_k" => DiffInstructStarWeight::Q3_K,
        "q4_0" => DiffInstructStarWeight::Q4_0,
        "q5_0" => DiffInstructStarWeight::Q5_0,
        "q6_k" => DiffInstructStarWeight::Q6_K,
        "q8_0" => DiffInstructStarWeight::Q8_0,
    })
}

pub(crate) fn normalize_choice(value: &str) -> String {
    value
        .trim()
        .replace(['-', '.', ' '], "_")
        .to_ascii_lowercase()
}

pub(crate) fn preset_guidance(preset: &Preset) -> Option<f32> {
    match preset {
        Preset::SDTurbo | Preset::SDXLTurbo1_0 => Some(0.0),
        Preset::JuggernautXL11 => Some(6.0),
        Preset::DreamShaperXL2_1Turbo => Some(2.0),
        Preset::SegmindVega => Some(9.0),
        _ => None,
    }
}
