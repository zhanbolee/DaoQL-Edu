// Copyright (c) 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
// SPDX-License-Identifier: BSL-1.1
//
// Licensed under the Business Source License, version 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
//
//     https://spdx.org/licenses/BSL-1.1.html
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//
//! DSL Parser (Recursive Descent)
//!
//! Educational Notes:
//! - recursive descent: each non-terminal corresponds to a parse function
//! - suitable for hand-written parser, code intuitive and easy to understand
//! - does not support left recursion (needs to be rewritten as recurrent)

use crate::dsl::ast::{DslQuery, FieldDef, FilterExpr, HistoryExpr, MutationOp};
use crate::dsl::lexer::{Lexer, Token};
use crate::error::DaoQLError;

/// Parse
pub struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    _input: &'a str,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Result<Self, DaoQLError> {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize()?;
        Ok(Self {
            tokens,
            pos: 0,
            _input: input,
        })
    }

    /// Parse DSL statement
    pub fn parse(&mut self) -> Result<DslQuery, DaoQLError> {
        self.skip_newlines();
        match self.current() {
            Token::Query => self.parse_query(),
            Token::Mutation => self.parse_mutation(),
            Token::Analyze => self.parse_analyze(),
            Token::Define => self.parse_define(),
            Token::Similar => self.parse_similar(),
            _ => Err(DaoQLError::DslParse(format!(
                "expect query/mutation/analyze/define/similar，get {:?}",
                self.current()
            ))),
        }
    }

    fn current(&self) -> Token {
        self.tokens.get(self.pos).cloned().unwrap_or(Token::EOF)
    }

    fn advance(&mut self) -> Token {
        let t = self.current();
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        t
    }

    fn expect(&mut self, expected: Token) -> Result<(), DaoQLError> {
        let actual = self.advance();
        if actual != expected {
            return Err(DaoQLError::DslParse(format!(
                "expect {:?}，get {:?}",
                expected, actual
            )));
        }
        Ok(())
    }

    fn skip_newlines(&mut self) {
        while self.current() == Token::Newline {
            self.advance();
        }
    }

    /// parse: query { Target(...) { ... } }
    fn parse_query(&mut self) -> Result<DslQuery, DaoQLError> {
        self.expect(Token::Query)?;
        self.expect(Token::LeftBrace)?;
        let target = self.parse_identifier()?;

        let mut filter = None;
        let mut projections = Vec::new();
        let mut limit = None;
        let mut history = None;
        let mut aggregate = None;

        // Parseparameterlist (filter: ..., limit: ...)
        if self.current() == Token::LeftParen {
            self.advance();
            while self.current() != Token::RightParen {
                let param = self.parse_identifier()?;
                self.expect(Token::Colon)?;
                match param.as_str() {
                    "filter" => filter = Some(self.parse_filter()?),
                    "limit" => limit = Some(self.parse_number()? as usize),
                    "history" => history = Some(self.parse_history()?),
                    "aggregate" => {
                        self.expect(Token::LeftBrace)?;
                        let agg_field = self.parse_identifier()?;
                        self.expect(Token::Colon)?;
                        let agg_op = self.parse_identifier()?;
                        self.expect(Token::RightBrace)?;
                        aggregate = Some((agg_field, agg_op));
                    }
                    _ => {
                        // skip unknown parameter
                        self.advance();
                    }
                }
                if self.current() == Token::Comma {
                    self.advance();
                }
            }
            self.expect(Token::RightParen)?;
        }

        // Parseproject { id, name }
        if self.current() == Token::LeftBrace {
            self.advance();
            while self.current() != Token::RightBrace {
                projections.push(self.parse_identifier()?);
                if self.current() == Token::Comma {
                    self.advance();
                }
            }
            self.expect(Token::RightBrace)?;
        }

        self.expect(Token::RightBrace)?;

        Ok(DslQuery::Query {
            target,
            filter,
            projections,
            limit,
            history,
            aggregate,
        })
    }

    fn parse_mutation(&mut self) -> Result<DslQuery, DaoQLError> {
        self.expect(Token::Mutation)?;
        self.expect(Token::LeftBrace)?;
        let op = match self.current() {
            Token::Create => { self.advance(); MutationOp::Create }
            Token::Update => { self.advance(); MutationOp::Update }
            Token::Delete => { self.advance(); MutationOp::Delete }
            _ => return Err(DaoQLError::DslParse("expect create/update/delete".to_string())),
        };
        let target = self.parse_identifier()?;
        self.expect(Token::LeftParen)?;
        self.expect(Token::Identifier("input".to_string()))?;
        self.expect(Token::Colon)?;
        self.expect(Token::LeftBrace)?;
        let mut input = Vec::new();
        while self.current() != Token::RightBrace {
            let key = self.parse_identifier()?;
            self.expect(Token::Colon)?;
            let value = self.parse_value()?;
            input.push((key, value));
            if self.current() == Token::Comma {
                self.advance();
            }
        }
        self.expect(Token::RightBrace)?;
        self.expect(Token::RightParen)?;
        self.expect(Token::RightBrace)?;

        Ok(DslQuery::Mutation { op, target, input })
    }

    fn parse_analyze(&mut self) -> Result<DslQuery, DaoQLError> {
        self.expect(Token::Analyze)?;
        self.expect(Token::LeftBrace)?;
        let algorithm = self.parse_identifier()?;
        self.expect(Token::Identifier("on".to_string()))?;
        let target = self.parse_identifier()?;
        let mut limit = None;
        if self.current() == Token::LeftParen {
            self.advance();
            self.expect(Token::Limit)?;
            self.expect(Token::Colon)?;
            limit = Some(self.parse_number()? as usize);
            self.expect(Token::RightParen)?;
        }
        self.expect(Token::LeftBrace)?;
        // skipproject
        while self.current() != Token::RightBrace {
            self.advance();
        }
        self.expect(Token::RightBrace)?;
        self.expect(Token::RightBrace)?;

        Ok(DslQuery::Analyze { algorithm, target, limit })
    }

    fn parse_define(&mut self) -> Result<DslQuery, DaoQLError> {
        self.expect(Token::Define)?;
        self.expect(Token::Type)?;
        let name = self.parse_identifier()?;
        self.expect(Token::LeftBrace)?;
        let mut fields = Vec::new();
        while self.current() != Token::RightBrace {
            self.expect(Token::Field)?;
            let field_name = self.parse_identifier()?;
            self.expect(Token::Colon)?;
            let field_type = self.parse_identifier()?;
            let mut required = false;
            let mut default = None;
            if self.current() == Token::LeftBrace {
                self.advance();
                while self.current() != Token::RightBrace {
                    let attr = self.parse_identifier()?;
                    match attr.as_str() {
                        "required" => required = true,
                        "default" => {
                            self.expect(Token::Colon)?;
                            default = Some(self.parse_value()?);
                        }
                        _ => {}
                    }
                    if self.current() == Token::Comma {
                        self.advance();
                    }
                }
                self.expect(Token::RightBrace)?;
            }
            fields.push(FieldDef {
                name: field_name,
                field_type,
                required,
                default,
            });
        }
        self.expect(Token::RightBrace)?;

        Ok(DslQuery::Define { name, fields })
    }

    fn parse_similar(&mut self) -> Result<DslQuery, DaoQLError> {
        self.expect(Token::Similar)?;
        self.expect(Token::LeftBrace)?;
        let target = self.parse_identifier()?;
        self.expect(Token::LeftParen)?;
        match self.advance() {
            Token::Query => {}
            Token::Identifier(s) if s == "query" => {}
            other => return Err(DaoQLError::DslParse(format!("expect query，get {:?}", other))),
        }
        self.expect(Token::Colon)?;
        let query_vector = self.parse_vector()?;
        self.expect(Token::Comma)?;
        self.expect(Token::Identifier("k".to_string()))?;
        self.expect(Token::Colon)?;
        let k = self.parse_number()? as usize;
        self.expect(Token::RightParen)?;
        self.expect(Token::LeftBrace)?;
        while self.current() != Token::RightBrace {
            self.advance();
        }
        self.expect(Token::RightBrace)?;
        self.expect(Token::RightBrace)?;

        Ok(DslQuery::Similar { target, query_vector, k })
    }

    fn parse_identifier(&mut self) -> Result<String, DaoQLError> {
        match self.advance() {
            Token::Identifier(s) => Ok(s),
            Token::Query => Ok("query".to_string()),
            Token::Mutation => Ok("mutation".to_string()),
            Token::Analyze => Ok("analyze".to_string()),
            Token::Define => Ok("define".to_string()),
            Token::Type => Ok("type".to_string()),
            Token::Field => Ok("field".to_string()),
            Token::Create => Ok("create".to_string()),
            Token::Update => Ok("update".to_string()),
            Token::Delete => Ok("delete".to_string()),
            Token::Where => Ok("where".to_string()),
            Token::Limit => Ok("limit".to_string()),
            Token::OrderBy => Ok("orderBy".to_string()),
            Token::GroupBy => Ok("groupBy".to_string()),
            Token::History => Ok("history".to_string()),
            Token::Similar => Ok("similar".to_string()),
            Token::Int => Ok("Int".to_string()),
            Token::Float => Ok("Float".to_string()),
            Token::String => Ok("String".to_string()),
            Token::Bool => Ok("Bool".to_string()),
            Token::Array => Ok("Array".to_string()),
            Token::Map => Ok("Map".to_string()),
            t => Err(DaoQLError::DslParse(format!("expect identifier, got {:?}", t))),
        }
    }

    fn parse_number(&mut self) -> Result<f64, DaoQLError> {
        match self.advance() {
            Token::Number(n) => Ok(n),
            t => Err(DaoQLError::DslParse(format!("expect number, got {:?}", t))),
        }
    }

    fn parse_value(&mut self) -> Result<serde_json::Value, DaoQLError> {
        match self.current() {
            Token::StringLiteral(s) => {
                self.advance();
                Ok(serde_json::Value::String(s))
            }
            Token::Number(n) => {
                self.advance();
                if n.fract() == 0.0 && n >= 0.0 && n <= (i64::MAX as f64) {
                    Ok(serde_json::json!(n as i64))
                } else {
                    Ok(serde_json::json!(n))
                }
            }
            Token::Identifier(s) if s == "true" => {
                self.advance();
                Ok(serde_json::Value::Bool(true))
            }
            Token::Identifier(s) if s == "false" => {
                self.advance();
                Ok(serde_json::Value::Bool(false))
            }
            _ => Ok(serde_json::Value::Null),
        }
    }

    fn parse_filter(&mut self) -> Result<FilterExpr, DaoQLError> {
        self.parse_filter_or()
    }

    fn parse_filter_or(&mut self) -> Result<FilterExpr, DaoQLError> {
        let mut left = self.parse_filter_and()?;
        while self.current() == Token::Or {
            self.advance();
            let right = self.parse_filter_and()?;
            left = FilterExpr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_filter_and(&mut self) -> Result<FilterExpr, DaoQLError> {
        let mut left = self.parse_filter_atom()?;
        while self.current() == Token::And {
            self.advance();
            let right = self.parse_filter_atom()?;
            left = FilterExpr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_filter_atom(&mut self) -> Result<FilterExpr, DaoQLError> {
        self.expect(Token::LeftBrace)?;
        let field = self.parse_identifier()?;
        let expr = match self.current() {
            Token::Colon => {
                self.advance();
                let value = self.parse_value()?;
                FilterExpr::Eq { field, value }
            }
            Token::Gt => {
                self.advance();
                let value = self.parse_value()?;
                FilterExpr::Gt { field, value }
            }
            Token::Lt => {
                self.advance();
                let value = self.parse_value()?;
                FilterExpr::Lt { field, value }
            }
            Token::Gte => {
                self.advance();
                let value = self.parse_value()?;
                FilterExpr::Gte { field, value }
            }
            Token::Lte => {
                self.advance();
                let value = self.parse_value()?;
                FilterExpr::Lte { field, value }
            }
            other => {
                return Err(DaoQLError::DslParse(format!(
                    "Filter condition expect : > < >= <=, got {:?}",
                    other
                )));
            }
        };
        self.expect(Token::RightBrace)?;
        Ok(expr)
    }

    fn parse_history(&mut self) -> Result<HistoryExpr, DaoQLError> {
        let func = self.parse_identifier()?;
        self.expect(Token::LeftParen)?;
        let arg = self.parse_number()? as i64;
        self.expect(Token::RightParen)?;
        Ok(match func.as_str() {
            "all" => HistoryExpr::All,
            "last" => HistoryExpr::Last(arg as usize),
            "at" => HistoryExpr::At(arg),
            _ => HistoryExpr::All,
        })
    }

    fn parse_vector(&mut self) -> Result<Vec<f32>, DaoQLError> {
        self.expect(Token::LeftBracket)?;
        let mut vec = Vec::new();
        while self.current() != Token::RightBracket {
            vec.push(self.parse_number()? as f32);
            if self.current() == Token::Comma {
                self.advance();
            }
        }
        self.expect(Token::RightBracket)?;
        Ok(vec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query() {
        let input = r#"query { Order(filter: {status: 1}, limit: 10) { id, name } }"#;
        let mut parser = Parser::new(input).unwrap();
        let ast = parser.parse().unwrap();
        match ast {
            DslQuery::Query { target, limit, .. } => {
                assert_eq!(target, "Order");
                assert_eq!(limit, Some(10));
            }
            _ => panic!("expected Query"),
        }
    }

    #[test]
    fn test_parse_define() {
        let input = r#"define type Order { field amount: Float { required } field status: Int { default: 0 } }"#;
        let mut parser = Parser::new(input).unwrap();
        let ast = parser.parse().unwrap();
        match ast {
            DslQuery::Define { name, fields } => {
                assert_eq!(name, "Order");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "amount");
                assert!(fields[0].required);
            }
            _ => panic!("expected Define"),
        }
    }

    #[test]
    fn test_parse_mutation() {
        let input = r#"mutation { create Order(input: { amount: 100.0 }) }"#;
        let mut parser = Parser::new(input).unwrap();
        let ast = parser.parse().unwrap();
        match ast {
            DslQuery::Mutation { op, target, .. } => {
                assert_eq!(op, MutationOp::Create);
                assert_eq!(target, "Order");
            }
            _ => panic!("expected Mutation"),
        }
    }

    #[test]
    fn test_parse_filter_gt_lt() {
        let input = r#"query { Order(filter: {age > 18}, limit: 10) { id } }"#;
        let mut parser = Parser::new(input).unwrap();
        let ast = parser.parse().unwrap();
        match ast {
            DslQuery::Query { filter, .. } => {
                assert_eq!(filter, Some(FilterExpr::Gt { field: "age".to_string(), value: serde_json::json!(18) }));
            }
            _ => panic!("expected Query"),
        }
    }

    #[test]
    fn test_parse_filter_and() {
        let input = r#"query { Order(filter: {status: 1} and {age > 18}, limit: 10) { id } }"#;
        let mut parser = Parser::new(input).unwrap();
        let ast = parser.parse().unwrap();
        match ast {
            DslQuery::Query { filter, .. } => {
                let expected = FilterExpr::And(
                    Box::new(FilterExpr::Eq { field: "status".to_string(), value: serde_json::json!(1) }),
                    Box::new(FilterExpr::Gt { field: "age".to_string(), value: serde_json::json!(18) }),
                );
                assert_eq!(filter, Some(expected));
            }
            _ => panic!("expected Query"),
        }
    }

    #[test]
    fn test_parse_filter_or() {
        let input = r#"query { Order(filter: {status: 1} or {age > 18}, limit: 10) { id } }"#;
        let mut parser = Parser::new(input).unwrap();
        let ast = parser.parse().unwrap();
        match ast {
            DslQuery::Query { filter, .. } => {
                let expected = FilterExpr::Or(
                    Box::new(FilterExpr::Eq { field: "status".to_string(), value: serde_json::json!(1) }),
                    Box::new(FilterExpr::Gt { field: "age".to_string(), value: serde_json::json!(18) }),
                );
                assert_eq!(filter, Some(expected));
            }
            _ => panic!("expected Query"),
        }
    }
}
