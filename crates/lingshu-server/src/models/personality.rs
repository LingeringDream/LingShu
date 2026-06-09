use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// L5: A personality state snapshot recording the 7 trait parameters at a point in time.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PersonalitySnapshot {
    pub id: Uuid,
    pub user_id: Uuid,
    pub trait_values: serde_json::Value,
    pub change_reason: Option<String>,
    pub source_memory_ids: Vec<Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// The 7 personality trait parameters (stored inside trait_values JSONB).
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PersonalityTraits {
    /// How direct vs. diplomatic (0.0 = diplomatic, 1.0 = direct)
    #[serde(default = "default_trait")]
    pub directness: f32,
    /// How warm vs. neutral (0.0 = neutral, 1.0 = warm)
    #[serde(default = "default_trait")]
    pub warmth: f32,
    /// How proactive vs. reactive (0.0 = reactive, 1.0 = proactive)
    #[serde(default = "default_trait")]
    pub proactivity: f32,
    /// How risk-tolerant (0.0 = cautious, 1.0 = bold)
    #[serde(default = "default_trait")]
    pub risk_tolerance: f32,
    /// How verbose (0.0 = concise, 1.0 = detailed)
    #[serde(default = "default_trait")]
    pub verbosity: f32,
    /// How formal (0.0 = casual, 1.0 = formal)
    #[serde(default = "default_trait")]
    pub formality: f32,
    /// How humorous (0.0 = serious, 1.0 = playful)
    #[serde(default = "default_trait")]
    pub humor: f32,
}

fn default_trait() -> f32 {
    0.5
}

impl Default for PersonalityTraits {
    fn default() -> Self {
        Self {
            directness: 0.5,
            warmth: 0.5,
            proactivity: 0.5,
            risk_tolerance: 0.5,
            verbosity: 0.5,
            formality: 0.5,
            humor: 0.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_traits_all_0_5() {
        let t = PersonalityTraits::default();
        assert!((t.directness - 0.5).abs() < f32::EPSILON);
        assert!((t.warmth - 0.5).abs() < f32::EPSILON);
        assert!((t.proactivity - 0.5).abs() < f32::EPSILON);
        assert!((t.risk_tolerance - 0.5).abs() < f32::EPSILON);
        assert!((t.verbosity - 0.5).abs() < f32::EPSILON);
        assert!((t.formality - 0.5).abs() < f32::EPSILON);
        assert!((t.humor - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn traits_json_round_trip() {
        let orig = PersonalityTraits {
            directness: 0.8,
            warmth: 0.6,
            proactivity: 0.3,
            risk_tolerance: 0.9,
            verbosity: 0.2,
            formality: 0.7,
            humor: 0.4,
        };
        let json = serde_json::to_value(&orig).unwrap();
        let restored: PersonalityTraits = serde_json::from_value(json).unwrap();
        assert!((restored.directness - 0.8).abs() < f32::EPSILON);
        assert!((restored.humor - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn traits_json_missing_fields_get_default() {
        let json = serde_json::json!({"warmth": 0.9});
        let t: PersonalityTraits = serde_json::from_value(json).unwrap();
        assert!((t.warmth - 0.9).abs() < f32::EPSILON);
        assert!((t.directness - 0.5).abs() < f32::EPSILON); // default
        assert!((t.humor - 0.5).abs() < f32::EPSILON); // default
    }
}
