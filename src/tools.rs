use alisp::Evaluator;
use alisp::expr_to_string;
use crate::security::SecurityPolicy;

pub struct AlispHost {
    eval: Evaluator,
    policy: Option<SecurityPolicy>,
}

impl AlispHost {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            eval: Evaluator::new(),
            policy: None,
        }
    }

    pub fn with_policy(policy: SecurityPolicy) -> Self {
        Self {
            eval: Evaluator::new(),
            policy: Some(policy),
        }
    }

    pub fn execute(&mut self, code: &str) -> Result<String, String> {
        if let Some(ref policy) = self.policy {
            policy.check_code(code)?;
            policy.confirm_dangerous(code)?;
        }

        match self.eval.eval_str(code) {
            Ok(Some(val)) => Ok(expr_to_string(&val)),
            Ok(None) => Ok("nil".to_string()),
            Err(e) => Err(e),
        }
    }
}
