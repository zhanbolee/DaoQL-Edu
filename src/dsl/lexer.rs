// Copyright (c) 2026 黎展波 / Atlas Lee <4859345@qq.com>
// SPDX-License-Identifier: BSL-1.1
//
// Licensed under the Business Source License, version 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
//
//     https://mariadb.com/bsl11/
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//
//! DSL 词法分析器
//!
//! 教学说明：
//! - 将输入字符串切分为 Token 序列
//! - 支持：关键字、标识符、字符串、数字、符号

use crate::error::DaoQLError;

/// Token 类型
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // 关键字
    Query,
    Mutation,
    Analyze,
    Define,
    Type,
    Field,
    Create,
    Update,
    Delete,
    Where,
    Limit,
    OrderBy,
    GroupBy,
    History,
    Similar,
    And,
    Or,
    Gt,
    Lt,
    Gte,
    Lte,
    // 类型
    Int,
    Float,
    String,
    Bool,
    Array,
    Map,
    // 符号
    LeftBrace,
    RightBrace,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    Colon,
    Comma,
    Semicolon,
    Equal,
    Arrow,
    At,
    // 字面量
    Identifier(String),
    StringLiteral(String),
    Number(f64),
    // 特殊
    Newline,
    EOF,
}

/// 词法分析器
pub struct Lexer<'a> {
    pos: usize,
    chars: Vec<char>,
    _phantom: std::marker::PhantomData<&'a str>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            pos: 0,
            chars: input.chars().collect(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// 获取下一个 Token
    pub fn next_token(&mut self) -> Result<Token, DaoQLError> {
        self.skip_whitespace();

        if self.pos >= self.chars.len() {
            return Ok(Token::EOF);
        }

        let ch = self.chars[self.pos];

        match ch {
            '{' => { self.pos += 1; Ok(Token::LeftBrace) }
            '}' => { self.pos += 1; Ok(Token::RightBrace) }
            '(' => { self.pos += 1; Ok(Token::LeftParen) }
            ')' => { self.pos += 1; Ok(Token::RightParen) }
            '[' => { self.pos += 1; Ok(Token::LeftBracket) }
            ']' => { self.pos += 1; Ok(Token::RightBracket) }
            ':' => { self.pos += 1; Ok(Token::Colon) }
            ',' => { self.pos += 1; Ok(Token::Comma) }
            ';' => { self.pos += 1; Ok(Token::Semicolon) }
            '=' => { self.pos += 1; Ok(Token::Equal) }
            '>' => {
                self.pos += 1;
                if self.peek() == Some('=') {
                    self.pos += 1;
                    Ok(Token::Gte)
                } else {
                    Ok(Token::Gt)
                }
            }
            '<' => {
                self.pos += 1;
                if self.peek() == Some('=') {
                    self.pos += 1;
                    Ok(Token::Lte)
                } else {
                    Ok(Token::Lt)
                }
            }
            '@' => { self.pos += 1; Ok(Token::At) }
            '\n' => { self.pos += 1; Ok(Token::Newline) }
            '"' | '\'' => self.read_string(ch),
            '-' if self.peek() == Some('>') => {
                self.pos += 2;
                Ok(Token::Arrow)
            }
            _ if ch.is_ascii_digit() => self.read_number(),
            _ if ch.is_alphabetic() || ch == '_' => self.read_identifier(),
            _ => Err(DaoQLError::DslParse(format!("非法字符: {}", ch))),
        }
    }

    /// 读取所有 Token
    pub fn tokenize(&mut self) -> Result<Vec<Token>, DaoQLError> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token()?;
            if token == Token::EOF {
                tokens.push(token);
                break;
            }
            tokens.push(token);
        }
        Ok(tokens)
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() {
            let ch = self.chars[self.pos];
            if ch == ' ' || ch == '\t' || ch == '\r' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn read_string(&mut self, quote: char) -> Result<Token, DaoQLError> {
        self.pos += 1; // 跳过左引号
        let start = self.pos;
        while self.pos < self.chars.len() && self.chars[self.pos] != quote {
            self.pos += 1;
        }
        if self.pos >= self.chars.len() {
            return Err(DaoQLError::DslParse("未闭合的字符串".to_string()));
        }
        let s: String = self.chars[start..self.pos].iter().collect();
        self.pos += 1; // 跳过右引号
        Ok(Token::StringLiteral(s))
    }

    fn read_number(&mut self) -> Result<Token, DaoQLError> {
        let start = self.pos;
        let mut has_dot = false;
        while self.pos < self.chars.len() {
            let ch = self.chars[self.pos];
            if ch.is_ascii_digit() {
                self.pos += 1;
            } else if ch == '.' && !has_dot {
                has_dot = true;
                self.pos += 1;
            } else {
                break;
            }
        }
        let s: String = self.chars[start..self.pos].iter().collect();
        let num = s.parse::<f64>()
            .map_err(|e| DaoQLError::DslParse(format!("数字解析失败: {e}")))?;
        Ok(Token::Number(num))
    }

    fn read_identifier(&mut self) -> Result<Token, DaoQLError> {
        let start = self.pos;
        while self.pos < self.chars.len() {
            let ch = self.chars[self.pos];
            if ch.is_alphanumeric() || ch == '_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let s: String = self.chars[start..self.pos].iter().collect();
        Ok(match s.as_str() {
            "query" => Token::Query,
            "mutation" => Token::Mutation,
            "analyze" => Token::Analyze,
            "define" => Token::Define,
            "type" => Token::Type,
            "field" => Token::Field,
            "create" => Token::Create,
            "update" => Token::Update,
            "delete" => Token::Delete,
            "where" => Token::Where,
            "limit" => Token::Limit,
            "orderBy" => Token::OrderBy,
            "groupBy" => Token::GroupBy,
            "history" => Token::History,
            "similar" => Token::Similar,
            "and" => Token::And,
            "or" => Token::Or,
            "Int" => Token::Int,
            "Float" => Token::Float,
            "String" => Token::String,
            "Bool" => Token::Bool,
            "Array" => Token::Array,
            "Map" => Token::Map,
            _ => Token::Identifier(s),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_basic() {
        let input = r#"query { Order(id: "abc") { id, name } }"#;
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::Query);
        assert_eq!(tokens[1], Token::LeftBrace);
    }

    #[test]
    fn test_lexer_keywords() {
        let input = "query mutation analyze define type field create update delete";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::Query);
        assert_eq!(tokens[1], Token::Mutation);
        assert_eq!(tokens[2], Token::Analyze);
    }

    #[test]
    fn test_lexer_string() {
        let input = r#""hello world" 'test'"#;
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::StringLiteral("hello world".to_string()));
        assert_eq!(tokens[1], Token::StringLiteral("test".to_string()));
    }

    #[test]
    fn test_lexer_number() {
        let input = "42 1.23";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::Number(42.0));
        assert_eq!(tokens[1], Token::Number(1.23));
    }
}
