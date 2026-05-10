// 計算プラグイン

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::error::AppError;
use crate::models::plugin::{ToolCall, ToolDefinition, ToolResult};
use crate::plugin::system::PluginHandler;

/// 計算プラグイン — 数式を計算する
pub struct CalculatorPlugin;

impl Default for CalculatorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl CalculatorPlugin {
    pub fn new() -> Self {
        Self
    }

    /// 簡易数式評価（四則演算のみ対応）
    fn evaluate(&self, expression: &str) -> Result<f64, String> {
        let expr = expression.trim();
        if expr.is_empty() {
            return Err("空の数式".to_string());
        }

        // 入力バリデーション: 許可文字のみ
        if !expr
            .chars()
            .all(|c| c.is_ascii_digit() || "+-*/.() ".contains(c))
        {
            return Err(format!("不正な文字が含まれている: {}", expr));
        }

        // 簡易パーサー: 加減算レベルから開始
        let tokens = self.tokenize(expr)?;
        let mut pos = 0;
        let result = self.parse_expr(&tokens, &mut pos)?;

        if pos < tokens.len() {
            return Err(format!("予期しないトークン: {:?}", tokens[pos]));
        }

        Ok(result)
    }

    fn tokenize(&self, expr: &str) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        let mut chars = expr.chars().peekable();

        while let Some(&ch) = chars.peek() {
            match ch {
                ' ' => {
                    chars.next();
                }
                '0'..='9' | '.' => {
                    let mut num_str = String::new();
                    while let Some(&c) = chars.peek() {
                        if c.is_ascii_digit() || c == '.' {
                            num_str.push(c);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    let num: f64 = num_str
                        .parse()
                        .map_err(|_| format!("数値パースエラー: {}", num_str))?;
                    tokens.push(Token::Number(num));
                }
                '+' => {
                    tokens.push(Token::Plus);
                    chars.next();
                }
                '-' => {
                    tokens.push(Token::Minus);
                    chars.next();
                }
                '*' => {
                    tokens.push(Token::Mul);
                    chars.next();
                }
                '/' => {
                    tokens.push(Token::Div);
                    chars.next();
                }
                '(' => {
                    tokens.push(Token::LParen);
                    chars.next();
                }
                ')' => {
                    tokens.push(Token::RParen);
                    chars.next();
                }
                _ => return Err(format!("不正な文字: {}", ch)),
            }
        }

        Ok(tokens)
    }

    /// expr = term (('+' | '-') term)*
    fn parse_expr(&self, tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
        let mut result = self.parse_term(tokens, pos)?;

        while *pos < tokens.len() {
            match tokens[*pos] {
                Token::Plus => {
                    *pos += 1;
                    result += self.parse_term(tokens, pos)?;
                }
                Token::Minus => {
                    *pos += 1;
                    result -= self.parse_term(tokens, pos)?;
                }
                _ => break,
            }
        }

        Ok(result)
    }

    /// term = factor (('*' | '/') factor)*
    fn parse_term(&self, tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
        let mut result = self.parse_factor(tokens, pos)?;

        while *pos < tokens.len() {
            match tokens[*pos] {
                Token::Mul => {
                    *pos += 1;
                    result *= self.parse_factor(tokens, pos)?;
                }
                Token::Div => {
                    *pos += 1;
                    let divisor = self.parse_factor(tokens, pos)?;
                    if divisor == 0.0 {
                        return Err("ゼロ除算".to_string());
                    }
                    result /= divisor;
                }
                _ => break,
            }
        }

        Ok(result)
    }

    /// factor = NUMBER | '(' expr ')' | '-' factor
    fn parse_factor(&self, tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
        if *pos >= tokens.len() {
            return Err("予期しない入力終了".to_string());
        }

        match tokens[*pos] {
            Token::Number(n) => {
                *pos += 1;
                Ok(n)
            }
            Token::LParen => {
                *pos += 1;
                let result = self.parse_expr(tokens, pos)?;
                if *pos >= tokens.len() || tokens[*pos] != Token::RParen {
                    return Err("閉じ括弧が見つからない".to_string());
                }
                *pos += 1;
                Ok(result)
            }
            Token::Minus => {
                *pos += 1;
                let result = self.parse_factor(tokens, pos)?;
                Ok(-result)
            }
            _ => Err(format!("予期しないトークン: {:?}", tokens[*pos])),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Plus,
    Minus,
    Mul,
    Div,
    LParen,
    RParen,
}

#[async_trait]
impl<R: tauri::Runtime> PluginHandler<R> for CalculatorPlugin {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "数式を計算する"
    }

    fn tools(&self) -> Vec<ToolDefinition> {
        vec![ToolDefinition {
            name: "calculate".to_string(),
            description: "数式を評価して結果を返す".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "評価する数式（四則演算対応）"
                    }
                },
                "required": ["expression"]
            }),
        }]
    }

    async fn execute(
        &self,
        tool_call: &ToolCall,
        _app_handle: &tauri::AppHandle<R>,
    ) -> Result<ToolResult, AppError> {
        let expression = tool_call
            .arguments
            .get("expression")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Plugin("'expression' パラメータが必要".to_string()))?;

        let result = match self.evaluate(expression) {
            Ok(value) => ToolResult {
                tool_call_id: tool_call.id.clone(),
                content: value.to_string(),
                is_error: false,
            },
            Err(err) => ToolResult {
                tool_call_id: tool_call.id.clone(),
                content: format!("計算エラー: {}", err),
                is_error: true,
            },
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tool_call(expression: &str) -> ToolCall {
        ToolCall {
            id: "test-call-1".to_string(),
            name: "calculate".to_string(),
            arguments: json!({ "expression": expression }),
            context: None,
        }
    }

    fn make_mock_app() -> tauri::App<tauri::test::MockRuntime> {
        tauri::test::mock_builder()
            .build(tauri::generate_context!())
            .unwrap()
    }

    #[test]
    fn test_plugin_metadata() {
        let plugin = CalculatorPlugin::new();
        let handler: &dyn PluginHandler<tauri::test::MockRuntime> = &plugin;
        assert_eq!(handler.name(), "calculator");
        assert_eq!(handler.description(), "数式を計算する");
        assert_eq!(handler.tools().len(), 1);
        assert_eq!(handler.tools()[0].name, "calculate");
    }

    #[test]
    fn test_basic_arithmetic() {
        let plugin = CalculatorPlugin::new();
        assert_eq!(plugin.evaluate("2 + 3").unwrap(), 5.0);
        assert_eq!(plugin.evaluate("10 - 4").unwrap(), 6.0);
        assert_eq!(plugin.evaluate("3 * 7").unwrap(), 21.0);
        assert_eq!(plugin.evaluate("20 / 4").unwrap(), 5.0);
    }

    #[test]
    fn test_operator_precedence() {
        let plugin = CalculatorPlugin::new();
        assert_eq!(plugin.evaluate("2 + 3 * 4").unwrap(), 14.0);
        assert_eq!(plugin.evaluate("(2 + 3) * 4").unwrap(), 20.0);
    }

    #[test]
    fn test_negative_numbers() {
        let plugin = CalculatorPlugin::new();
        assert_eq!(plugin.evaluate("-5").unwrap(), -5.0);
        assert_eq!(plugin.evaluate("-3 + 7").unwrap(), 4.0);
    }

    #[test]
    fn test_division_by_zero() {
        let plugin = CalculatorPlugin::new();
        assert!(plugin.evaluate("1 / 0").is_err());
    }

    #[test]
    fn test_invalid_expression() {
        let plugin = CalculatorPlugin::new();
        assert!(plugin.evaluate("").is_err());
        assert!(plugin.evaluate("abc").is_err());
    }

    #[tokio::test]
    async fn test_execute_success() {
        let app = make_mock_app();
        let plugin = CalculatorPlugin::new();
        let tool_call = make_tool_call("2 + 3");
        let result = plugin.execute(&tool_call, app.handle()).await.unwrap();
        assert_eq!(result.content, "5");
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_execute_error() {
        let app = make_mock_app();
        let plugin = CalculatorPlugin::new();
        let tool_call = make_tool_call("1 / 0");
        let result = plugin.execute(&tool_call, app.handle()).await.unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("計算エラー"));
    }

    #[tokio::test]
    async fn test_execute_missing_param() {
        let app = make_mock_app();
        let plugin = CalculatorPlugin::new();
        let tool_call = ToolCall {
            id: "test-call-2".to_string(),
            name: "calculate".to_string(),
            arguments: json!({}),
            context: None,
        };
        let result = plugin.execute(&tool_call, app.handle()).await;
        assert!(result.is_err());
    }
}
