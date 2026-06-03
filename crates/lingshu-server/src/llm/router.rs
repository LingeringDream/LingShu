/// Model routing based on intent and complexity
pub struct ModelRouter {
    default_model: String,
}

impl ModelRouter {
    pub fn new(default_model: &str) -> Self {
        Self {
            default_model: default_model.to_string(),
        }
    }

    /// Route to the appropriate model based on task complexity
    pub fn route(&self, complexity: f32) -> &str {
        if complexity < 0.3 {
            "qwen2.5:1.5b" // Simple tasks use small model
        } else if complexity < 0.7 {
            &self.default_model // Medium tasks use default
        } else {
            "gpt-4o" // Complex tasks use large model (cloud)
        }
    }

    /// Estimate task complexity from message content
    pub fn estimate_complexity(message: &str) -> f32 {
        let len = message.len() as f32;
        if len < 50.0 {
            0.1 // Short messages are usually simple
        } else if len < 200.0 {
            0.4
        } else {
            0.7 // Long messages need more reasoning
        }
    }
}
