use diffusion_rs::api::{
    ClipSkip, LoraModeType, Prediction, RngFunction, SampleMethod, Scheduler, WeightType,
};
use serde_json::Value;

use crate::GenerateError;

pub(crate) fn string_field(
    request: &Value,
    field: &'static str,
) -> Result<Option<String>, GenerateError> {
    match request.get(field) {
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(GenerateError::InvalidField {
            field,
            message: "expected string".to_owned(),
        }),
        None => Ok(None),
    }
}

pub(crate) fn integer_field(
    request: &Value,
    field: &'static str,
) -> Result<Option<i64>, GenerateError> {
    match request.get(field) {
        Some(Value::Number(value)) => {
            value
                .as_i64()
                .map(Some)
                .ok_or_else(|| GenerateError::InvalidField {
                    field,
                    message: "expected integer".to_owned(),
                })
        }
        Some(_) => Err(GenerateError::InvalidField {
            field,
            message: "expected integer".to_owned(),
        }),
        None => Ok(None),
    }
}

pub(crate) fn number_field(
    request: &Value,
    field: &'static str,
) -> Result<Option<f64>, GenerateError> {
    match request.get(field) {
        Some(Value::Number(value)) => {
            value
                .as_f64()
                .map(Some)
                .ok_or_else(|| GenerateError::InvalidField {
                    field,
                    message: "expected number".to_owned(),
                })
        }
        Some(_) => Err(GenerateError::InvalidField {
            field,
            message: "expected number".to_owned(),
        }),
        None => Ok(None),
    }
}

pub(crate) fn bool_field(
    request: &Value,
    field: &'static str,
) -> Result<Option<bool>, GenerateError> {
    match request.get(field) {
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(GenerateError::InvalidField {
            field,
            message: "expected boolean".to_owned(),
        }),
        None => Ok(None),
    }
}

pub(crate) fn integer_array_field(
    request: &Value,
    field: &'static str,
) -> Result<Option<Vec<i32>>, GenerateError> {
    match request.get(field) {
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| match item {
                Value::Number(value) => value
                    .as_i64()
                    .ok_or_else(|| GenerateError::InvalidField {
                        field,
                        message: "expected an array of integers".to_owned(),
                    })
                    .and_then(|value| validate_i32_range(field, value, i32::MIN, i32::MAX)),
                _ => Err(GenerateError::InvalidField {
                    field,
                    message: "expected an array of integers".to_owned(),
                }),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        Some(_) => Err(GenerateError::InvalidField {
            field,
            message: "expected an array of integers".to_owned(),
        }),
        None => Ok(None),
    }
}

pub(crate) fn number_array_field(
    request: &Value,
    field: &'static str,
) -> Result<Option<Vec<f32>>, GenerateError> {
    match request.get(field) {
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| match item {
                Value::Number(value) => value.as_f64().map(|value| value as f32).ok_or_else(|| {
                    GenerateError::InvalidField {
                        field,
                        message: "expected an array of numbers".to_owned(),
                    }
                }),
                _ => Err(GenerateError::InvalidField {
                    field,
                    message: "expected an array of numbers".to_owned(),
                }),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        Some(_) => Err(GenerateError::InvalidField {
            field,
            message: "expected an array of numbers".to_owned(),
        }),
        None => Ok(None),
    }
}

pub(crate) fn integer_pair_field(
    request: &Value,
    field: &'static str,
) -> Result<Option<(i32, i32)>, GenerateError> {
    let Some(values) = integer_array_field(request, field)? else {
        return Ok(None);
    };
    if values.len() != 2 || values.iter().any(|value| *value <= 0) {
        return Err(GenerateError::InvalidField {
            field,
            message: "expected two positive integers".to_owned(),
        });
    }
    Ok(Some((values[0], values[1])))
}

pub(crate) fn number_pair_field(
    request: &Value,
    field: &'static str,
) -> Result<Option<(f32, f32)>, GenerateError> {
    let Some(values) = number_array_field(request, field)? else {
        return Ok(None);
    };
    if values.len() != 2 {
        return Err(GenerateError::InvalidField {
            field,
            message: "expected two numbers".to_owned(),
        });
    }
    Ok(Some((values[0], values[1])))
}

pub(crate) fn validate_dimension(field: &'static str, value: i64) -> Result<i32, GenerateError> {
    let value = validate_i32_range(field, value, 1, 4096)?;
    if value % 8 != 0 {
        return Err(GenerateError::InvalidField {
            field,
            message: "must be a multiple of 8".to_owned(),
        });
    }
    Ok(value)
}

pub(crate) fn validate_i32_range(
    field: &'static str,
    value: i64,
    min: i32,
    max: i32,
) -> Result<i32, GenerateError> {
    let value = i32::try_from(value).map_err(|_| GenerateError::InvalidField {
        field,
        message: format!("must be between {min} and {max}"),
    })?;
    if value < min || value > max {
        return Err(GenerateError::InvalidField {
            field,
            message: format!("must be between {min} and {max}"),
        });
    }
    Ok(value)
}

pub(crate) fn parse_clip_skip(value: i64) -> Result<ClipSkip, GenerateError> {
    match value {
        value if value <= 0 => Ok(ClipSkip::Unspecified),
        1 => Ok(ClipSkip::None),
        2 => Ok(ClipSkip::OneLayer),
        _ => Err(GenerateError::InvalidField {
            field: "clip_skip",
            message: "expected -1, 1, or 2".to_owned(),
        }),
    }
}

pub(crate) fn parse_sampling_method(value: &str) -> Result<SampleMethod, GenerateError> {
    match value {
        "euler" => Ok(SampleMethod::EULER_SAMPLE_METHOD),
        "euler_a" => Ok(SampleMethod::EULER_A_SAMPLE_METHOD),
        "heun" => Ok(SampleMethod::HEUN_SAMPLE_METHOD),
        "dpm2" => Ok(SampleMethod::DPM2_SAMPLE_METHOD),
        "dpmpp2s_a" => Ok(SampleMethod::DPMPP2S_A_SAMPLE_METHOD),
        "dpmpp2m" => Ok(SampleMethod::DPMPP2M_SAMPLE_METHOD),
        "dpmpp2mv2" => Ok(SampleMethod::DPMPP2Mv2_SAMPLE_METHOD),
        "ipndm" => Ok(SampleMethod::IPNDM_SAMPLE_METHOD),
        "ipndm_v" => Ok(SampleMethod::IPNDM_V_SAMPLE_METHOD),
        "lcm" => Ok(SampleMethod::LCM_SAMPLE_METHOD),
        "ddim_trailing" => Ok(SampleMethod::DDIM_TRAILING_SAMPLE_METHOD),
        "tcd" => Ok(SampleMethod::TCD_SAMPLE_METHOD),
        "res_multistep" => Ok(SampleMethod::RES_MULTISTEP_SAMPLE_METHOD),
        "res_2s" => Ok(SampleMethod::RES_2S_SAMPLE_METHOD),
        _ => Err(GenerateError::InvalidField {
            field: "sampling_method",
            message: format!("unknown sampler `{value}`"),
        }),
    }
}

pub(crate) fn parse_scheduler(value: &str) -> Result<Scheduler, GenerateError> {
    match value {
        "discrete" => Ok(Scheduler::DISCRETE_SCHEDULER),
        "karras" => Ok(Scheduler::KARRAS_SCHEDULER),
        "exponential" => Ok(Scheduler::EXPONENTIAL_SCHEDULER),
        "ays" => Ok(Scheduler::AYS_SCHEDULER),
        "gits" => Ok(Scheduler::GITS_SCHEDULER),
        "sgm_uniform" => Ok(Scheduler::SGM_UNIFORM_SCHEDULER),
        "simple" => Ok(Scheduler::SIMPLE_SCHEDULER),
        "smoothstep" => Ok(Scheduler::SMOOTHSTEP_SCHEDULER),
        "kl_optimal" => Ok(Scheduler::KL_OPTIMAL_SCHEDULER),
        "lcm" => Ok(Scheduler::LCM_SCHEDULER),
        "bong_tangent" => Ok(Scheduler::BONG_TANGENT_SCHEDULER),
        _ => Err(GenerateError::InvalidField {
            field: "scheduler",
            message: format!("unknown scheduler `{value}`"),
        }),
    }
}

pub(crate) fn parse_weight_type(value: &str) -> Result<WeightType, GenerateError> {
    match value {
        "f32" => Ok(WeightType::SD_TYPE_F32),
        "f16" => Ok(WeightType::SD_TYPE_F16),
        "bf16" => Ok(WeightType::SD_TYPE_BF16),
        "q8_0" => Ok(WeightType::SD_TYPE_Q8_0),
        "q8_1" => Ok(WeightType::SD_TYPE_Q8_1),
        "q4_0" => Ok(WeightType::SD_TYPE_Q4_0),
        "q4_1" => Ok(WeightType::SD_TYPE_Q4_1),
        "q4_k" => Ok(WeightType::SD_TYPE_Q4_K),
        "q5_0" => Ok(WeightType::SD_TYPE_Q5_0),
        "q5_1" => Ok(WeightType::SD_TYPE_Q5_1),
        "q5_k" => Ok(WeightType::SD_TYPE_Q5_K),
        "q6_k" => Ok(WeightType::SD_TYPE_Q6_K),
        "q2_k" => Ok(WeightType::SD_TYPE_Q2_K),
        "q3_k" => Ok(WeightType::SD_TYPE_Q3_K),
        "q8_k" => Ok(WeightType::SD_TYPE_Q8_K),
        "iq2_xxs" => Ok(WeightType::SD_TYPE_IQ2_XXS),
        "iq2_xs" => Ok(WeightType::SD_TYPE_IQ2_XS),
        "iq3_xxs" => Ok(WeightType::SD_TYPE_IQ3_XXS),
        "iq1_s" => Ok(WeightType::SD_TYPE_IQ1_S),
        "iq4_nl" => Ok(WeightType::SD_TYPE_IQ4_NL),
        "iq3_s" => Ok(WeightType::SD_TYPE_IQ3_S),
        "iq2_s" => Ok(WeightType::SD_TYPE_IQ2_S),
        "iq4_xs" => Ok(WeightType::SD_TYPE_IQ4_XS),
        "i8" => Ok(WeightType::SD_TYPE_I8),
        "i16" => Ok(WeightType::SD_TYPE_I16),
        "i32" => Ok(WeightType::SD_TYPE_I32),
        "i64" => Ok(WeightType::SD_TYPE_I64),
        "f64" => Ok(WeightType::SD_TYPE_F64),
        "iq1_m" => Ok(WeightType::SD_TYPE_IQ1_M),
        "tq1_0" => Ok(WeightType::SD_TYPE_TQ1_0),
        "tq2_0" => Ok(WeightType::SD_TYPE_TQ2_0),
        "mxfp4" => Ok(WeightType::SD_TYPE_MXFP4),
        _ => Err(GenerateError::InvalidField {
            field: "weight_type",
            message: format!("unknown weight type `{value}`"),
        }),
    }
}

pub(crate) fn parse_prediction(value: &str) -> Result<Prediction, GenerateError> {
    match value {
        "eps" => Ok(Prediction::EPS_PRED),
        "v" => Ok(Prediction::V_PRED),
        "edm_v" => Ok(Prediction::EDM_V_PRED),
        "sd3_flow" => Ok(Prediction::FLOW_PRED),
        "flux_flow" => Ok(Prediction::FLUX_FLOW_PRED),
        "flux2_flow" => Ok(Prediction::FLUX2_FLOW_PRED),
        _ => Err(GenerateError::InvalidField {
            field: "prediction",
            message: format!("unknown prediction `{value}`"),
        }),
    }
}

pub(crate) fn parse_rng_function(
    field: &'static str,
    value: &str,
) -> Result<RngFunction, GenerateError> {
    match value {
        "std_default" => Ok(RngFunction::STD_DEFAULT_RNG),
        "cuda" => Ok(RngFunction::CUDA_RNG),
        "cpu" => Ok(RngFunction::CPU_RNG),
        _ => Err(GenerateError::InvalidField {
            field,
            message: format!("unknown rng `{value}`"),
        }),
    }
}

pub(crate) fn parse_lora_apply_mode(value: &str) -> Result<LoraModeType, GenerateError> {
    match value {
        "auto" => Ok(LoraModeType::LORA_APPLY_AUTO),
        "immediately" => Ok(LoraModeType::LORA_APPLY_IMMEDIATELY),
        "at_runtime" => Ok(LoraModeType::LORA_APPLY_AT_RUNTIME),
        _ => Err(GenerateError::InvalidField {
            field: "lora_apply_mode",
            message: format!("unknown lora apply mode `{value}`"),
        }),
    }
}
