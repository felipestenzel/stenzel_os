//! JavaScript Engine
//!
//! Basic JavaScript interpreter for browser.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::str::FromStr;
use super::dom::{Dom, DomNode};

// Math helper functions for no_std
fn floor_f64(x: f64) -> f64 {
    let xi = x as i64;
    if x < 0.0 && x != xi as f64 {
        (xi - 1) as f64
    } else {
        xi as f64
    }
}

fn ceil_f64(x: f64) -> f64 {
    let xi = x as i64;
    if x > 0.0 && x != xi as f64 {
        (xi + 1) as f64
    } else {
        xi as f64
    }
}

fn round_f64(x: f64) -> f64 {
    floor_f64(x + 0.5)
}

/// JavaScript engine
pub struct JsEngine {
    /// Global context
    global: JsContext,
    /// DOM reference
    dom: Option<*const Dom>,
    /// Registered event handlers
    event_handlers: BTreeMap<String, Vec<JsFunction>>,
    /// Pending timeouts
    timeouts: Vec<Timeout>,
    /// Next timeout ID
    next_timeout_id: u32,
    /// Console output
    console_log: Vec<String>,
}

impl JsEngine {
    /// Create new JavaScript engine
    pub fn new() -> Self {
        let mut global = JsContext::new();

        // Add built-in functions
        Self::add_builtins(&mut global);

        Self {
            global,
            dom: None,
            event_handlers: BTreeMap::new(),
            timeouts: Vec::new(),
            next_timeout_id: 1,
            console_log: Vec::new(),
        }
    }

    /// Set DOM reference
    pub fn set_dom(&mut self, dom: &Dom) {
        self.dom = Some(dom as *const Dom);
    }

    /// Execute JavaScript code
    pub fn execute(&mut self, code: &str) -> Result<JsValue, JsError> {
        let tokens = Lexer::new(code).tokenize()?;
        let ast = Parser::new(&tokens).parse()?;
        self.evaluate(&ast, &mut self.global.clone())
    }

    /// Execute with callback for DOM modifications
    pub fn execute_with_dom<F>(&mut self, code: &str, _dom_callback: F) -> Result<JsValue, JsError>
    where
        F: FnMut(&str, JsValue),
    {
        self.execute(code)
    }

    /// Register event handler
    pub fn add_event_handler(&mut self, event: &str, handler: JsFunction) {
        self.event_handlers
            .entry(String::from(event))
            .or_insert_with(Vec::new)
            .push(handler);
    }

    /// Trigger event
    pub fn trigger_event(&mut self, event: &str, event_data: JsValue) -> Result<(), JsError> {
        if let Some(handlers) = self.event_handlers.get(event) {
            for handler in handlers.clone() {
                self.call_function(&handler, &[event_data.clone()])?;
            }
        }
        Ok(())
    }

    /// Set timeout
    pub fn set_timeout(&mut self, callback: JsFunction, delay_ms: u32) -> u32 {
        let id = self.next_timeout_id;
        self.next_timeout_id += 1;

        self.timeouts.push(Timeout {
            id,
            callback,
            fire_at: crate::time::uptime_secs() * 1000 + delay_ms as u64,
            is_interval: false,
        });

        id
    }

    /// Set interval
    pub fn set_interval(&mut self, callback: JsFunction, interval_ms: u32) -> u32 {
        let id = self.next_timeout_id;
        self.next_timeout_id += 1;

        self.timeouts.push(Timeout {
            id,
            callback,
            fire_at: crate::time::uptime_secs() * 1000 + interval_ms as u64,
            is_interval: true,
        });

        id
    }

    /// Clear timeout/interval
    pub fn clear_timeout(&mut self, id: u32) {
        self.timeouts.retain(|t| t.id != id);
    }

    /// Process pending timeouts
    pub fn process_timeouts(&mut self) -> Result<(), JsError> {
        let now = crate::time::uptime_secs() * 1000;
        let mut to_fire = Vec::new();

        for timeout in &self.timeouts {
            if timeout.fire_at <= now {
                to_fire.push(timeout.clone());
            }
        }

        // Remove non-interval timeouts
        self.timeouts.retain(|t| {
            t.fire_at > now || t.is_interval
        });

        // Fire callbacks
        for timeout in to_fire {
            self.call_function(&timeout.callback, &[])?;

            // Reschedule intervals
            if timeout.is_interval {
                for t in &mut self.timeouts {
                    if t.id == timeout.id {
                        t.fire_at = now + 1000; // Assume 1 second interval
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Get console log output
    pub fn console_output(&self) -> &[String] {
        &self.console_log
    }

    /// Clear console log
    pub fn clear_console(&mut self) {
        self.console_log.clear();
    }

    fn add_builtins(ctx: &mut JsContext) {
        // console object
        let mut console = JsObject::new();
        console.properties.insert(String::from("log"), JsValue::NativeFunction(native_console_log));
        console.properties.insert(String::from("error"), JsValue::NativeFunction(native_console_log));
        console.properties.insert(String::from("warn"), JsValue::NativeFunction(native_console_log));
        console.properties.insert(String::from("info"), JsValue::NativeFunction(native_console_log));
        ctx.set("console", JsValue::Object(console));

        // Math object
        let mut math = JsObject::new();
        math.properties.insert(String::from("PI"), JsValue::Number(3.14159265358979));
        math.properties.insert(String::from("E"), JsValue::Number(2.71828182845904));
        math.properties.insert(String::from("abs"), JsValue::NativeFunction(native_math_abs));
        math.properties.insert(String::from("floor"), JsValue::NativeFunction(native_math_floor));
        math.properties.insert(String::from("ceil"), JsValue::NativeFunction(native_math_ceil));
        math.properties.insert(String::from("round"), JsValue::NativeFunction(native_math_round));
        math.properties.insert(String::from("max"), JsValue::NativeFunction(native_math_max));
        math.properties.insert(String::from("min"), JsValue::NativeFunction(native_math_min));
        math.properties.insert(String::from("random"), JsValue::NativeFunction(native_math_random));
        ctx.set("Math", JsValue::Object(math));

        // Global functions
        ctx.set("parseInt", JsValue::NativeFunction(native_parse_int));
        ctx.set("parseFloat", JsValue::NativeFunction(native_parse_float));
        ctx.set("isNaN", JsValue::NativeFunction(native_is_nan));
        ctx.set("isFinite", JsValue::NativeFunction(native_is_finite));

        // JSON object
        let mut json = JsObject::new();
        json.properties.insert(String::from("parse"), JsValue::NativeFunction(native_json_parse));
        json.properties.insert(String::from("stringify"), JsValue::NativeFunction(native_json_stringify));
        ctx.set("JSON", JsValue::Object(json));
    }

    fn evaluate(&mut self, node: &AstNode, ctx: &mut JsContext) -> Result<JsValue, JsError> {
        match node {
            AstNode::Program(statements) => {
                let mut result = JsValue::Undefined;
                for stmt in statements {
                    result = self.evaluate(stmt, ctx)?;
                }
                Ok(result)
            }

            AstNode::Number(n) => Ok(JsValue::Number(*n)),
            AstNode::String(s) => Ok(JsValue::String(s.clone())),
            AstNode::Boolean(b) => Ok(JsValue::Boolean(*b)),
            AstNode::Null => Ok(JsValue::Null),
            AstNode::Undefined => Ok(JsValue::Undefined),

            AstNode::Identifier(name) => {
                ctx.get(name).ok_or_else(|| JsError::ReferenceError(alloc::format!("{} is not defined", name)))
            }

            AstNode::BinaryOp { op, left, right } => {
                let left_val = self.evaluate(left, ctx)?;
                let right_val = self.evaluate(right, ctx)?;
                self.binary_op(op, left_val, right_val)
            }

            AstNode::UnaryOp { op, operand } => {
                let val = self.evaluate(operand, ctx)?;
                self.unary_op(op, val)
            }

            AstNode::Assignment { target, value } => {
                let val = self.evaluate(value, ctx)?;
                if let AstNode::Identifier(name) = target.as_ref() {
                    ctx.set(name, val.clone());
                }
                Ok(val)
            }

            AstNode::VarDecl { name, init, .. } => {
                let val = if let Some(init_expr) = init {
                    self.evaluate(init_expr, ctx)?
                } else {
                    JsValue::Undefined
                };
                ctx.set(name, val);
                Ok(JsValue::Undefined)
            }

            AstNode::FunctionDecl { name, params, body } => {
                let func = JsFunction {
                    name: name.clone(),
                    params: params.clone(),
                    body: body.clone(),
                };
                ctx.set(name, JsValue::Function(func));
                Ok(JsValue::Undefined)
            }

            AstNode::FunctionExpr { params, body } => {
                Ok(JsValue::Function(JsFunction {
                    name: String::from("anonymous"),
                    params: params.clone(),
                    body: body.clone(),
                }))
            }

            AstNode::Call { callee, args } => {
                let callee_val = self.evaluate(callee, ctx)?;
                let arg_vals: Vec<JsValue> = args
                    .iter()
                    .map(|a| self.evaluate(a, ctx))
                    .collect::<Result<_, _>>()?;

                match callee_val {
                    JsValue::Function(func) => self.call_function(&func, &arg_vals),
                    JsValue::NativeFunction(f) => f(&arg_vals),
                    _ => Err(JsError::TypeError(String::from("is not a function"))),
                }
            }

            AstNode::If { condition, then_branch, else_branch } => {
                let cond = self.evaluate(condition, ctx)?;
                if cond.to_boolean() {
                    self.evaluate(then_branch, ctx)
                } else if let Some(else_b) = else_branch {
                    self.evaluate(else_b, ctx)
                } else {
                    Ok(JsValue::Undefined)
                }
            }

            AstNode::While { condition, body } => {
                let mut result = JsValue::Undefined;
                loop {
                    let cond = self.evaluate(condition, ctx)?;
                    if !cond.to_boolean() {
                        break;
                    }
                    result = self.evaluate(body, ctx)?;
                }
                Ok(result)
            }

            AstNode::For { init, condition, update, body } => {
                let mut local_ctx = ctx.clone();
                if let Some(init_expr) = init {
                    self.evaluate(init_expr, &mut local_ctx)?;
                }
                let mut result = JsValue::Undefined;
                loop {
                    if let Some(cond) = condition {
                        let cond_val = self.evaluate(cond, &mut local_ctx)?;
                        if !cond_val.to_boolean() {
                            break;
                        }
                    }
                    result = self.evaluate(body, &mut local_ctx)?;
                    if let Some(upd) = update {
                        self.evaluate(upd, &mut local_ctx)?;
                    }
                }
                Ok(result)
            }

            AstNode::Return { value } => {
                let val = if let Some(v) = value {
                    self.evaluate(v, ctx)?
                } else {
                    JsValue::Undefined
                };
                Err(JsError::Return(val))
            }

            AstNode::Block(statements) => {
                let mut result = JsValue::Undefined;
                for stmt in statements {
                    result = self.evaluate(stmt, ctx)?;
                }
                Ok(result)
            }

            AstNode::Array(elements) => {
                let vals: Vec<JsValue> = elements
                    .iter()
                    .map(|e| self.evaluate(e, ctx))
                    .collect::<Result<_, _>>()?;
                Ok(JsValue::Array(vals))
            }

            AstNode::Object(props) => {
                let mut obj = JsObject::new();
                for (key, val) in props {
                    let v = self.evaluate(val, ctx)?;
                    obj.properties.insert(key.clone(), v);
                }
                Ok(JsValue::Object(obj))
            }

            AstNode::MemberAccess { object, property } => {
                let obj_val = self.evaluate(object, ctx)?;
                match obj_val {
                    JsValue::Object(obj) => {
                        Ok(obj.properties.get(property).cloned().unwrap_or(JsValue::Undefined))
                    }
                    JsValue::Array(arr) => {
                        if property == "length" {
                            Ok(JsValue::Number(arr.len() as f64))
                        } else if let Ok(idx) = property.parse::<usize>() {
                            Ok(arr.get(idx).cloned().unwrap_or(JsValue::Undefined))
                        } else {
                            Ok(JsValue::Undefined)
                        }
                    }
                    JsValue::String(s) => {
                        if property == "length" {
                            Ok(JsValue::Number(s.len() as f64))
                        } else {
                            Ok(JsValue::Undefined)
                        }
                    }
                    _ => Ok(JsValue::Undefined),
                }
            }

            AstNode::IndexAccess { object, index } => {
                let obj_val = self.evaluate(object, ctx)?;
                let idx_val = self.evaluate(index, ctx)?;

                match (obj_val, idx_val) {
                    (JsValue::Array(arr), JsValue::Number(n)) => {
                        let idx = n as usize;
                        Ok(arr.get(idx).cloned().unwrap_or(JsValue::Undefined))
                    }
                    (JsValue::Object(obj), JsValue::String(key)) => {
                        Ok(obj.properties.get(&key).cloned().unwrap_or(JsValue::Undefined))
                    }
                    _ => Ok(JsValue::Undefined),
                }
            }

            AstNode::Ternary { condition, then_expr, else_expr } => {
                let cond = self.evaluate(condition, ctx)?;
                if cond.to_boolean() {
                    self.evaluate(then_expr, ctx)
                } else {
                    self.evaluate(else_expr, ctx)
                }
            }

            _ => Ok(JsValue::Undefined),
        }
    }

    fn call_function(&mut self, func: &JsFunction, args: &[JsValue]) -> Result<JsValue, JsError> {
        let mut local_ctx = self.global.clone();

        // Bind arguments
        for (i, param) in func.params.iter().enumerate() {
            let val = args.get(i).cloned().unwrap_or(JsValue::Undefined);
            local_ctx.set(param, val);
        }

        // Execute body
        match self.evaluate(&func.body, &mut local_ctx) {
            Ok(val) => Ok(val),
            Err(JsError::Return(val)) => Ok(val),
            Err(e) => Err(e),
        }
    }

    fn binary_op(&self, op: &str, left: JsValue, right: JsValue) -> Result<JsValue, JsError> {
        match op {
            "+" => {
                match (&left, &right) {
                    (JsValue::String(a), _) => Ok(JsValue::String(alloc::format!("{}{}", a, right.to_string()))),
                    (_, JsValue::String(b)) => Ok(JsValue::String(alloc::format!("{}{}", left.to_string(), b))),
                    _ => Ok(JsValue::Number(left.to_number() + right.to_number())),
                }
            }
            "-" => Ok(JsValue::Number(left.to_number() - right.to_number())),
            "*" => Ok(JsValue::Number(left.to_number() * right.to_number())),
            "/" => Ok(JsValue::Number(left.to_number() / right.to_number())),
            "%" => Ok(JsValue::Number(left.to_number() % right.to_number())),
            "==" => Ok(JsValue::Boolean(left.equals(&right))),
            "===" => Ok(JsValue::Boolean(left.strict_equals(&right))),
            "!=" => Ok(JsValue::Boolean(!left.equals(&right))),
            "!==" => Ok(JsValue::Boolean(!left.strict_equals(&right))),
            "<" => Ok(JsValue::Boolean(left.to_number() < right.to_number())),
            "<=" => Ok(JsValue::Boolean(left.to_number() <= right.to_number())),
            ">" => Ok(JsValue::Boolean(left.to_number() > right.to_number())),
            ">=" => Ok(JsValue::Boolean(left.to_number() >= right.to_number())),
            "&&" => Ok(if left.to_boolean() { right } else { left }),
            "||" => Ok(if left.to_boolean() { left } else { right }),
            _ => Err(JsError::SyntaxError(alloc::format!("Unknown operator: {}", op))),
        }
    }

    fn unary_op(&self, op: &str, val: JsValue) -> Result<JsValue, JsError> {
        match op {
            "!" => Ok(JsValue::Boolean(!val.to_boolean())),
            "-" => Ok(JsValue::Number(-val.to_number())),
            "+" => Ok(JsValue::Number(val.to_number())),
            "typeof" => Ok(JsValue::String(val.type_name())),
            _ => Err(JsError::SyntaxError(alloc::format!("Unknown operator: {}", op))),
        }
    }
}

impl Default for JsEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// JavaScript value
#[derive(Debug, Clone)]
pub enum JsValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    Array(Vec<JsValue>),
    Object(JsObject),
    Function(JsFunction),
    NativeFunction(fn(&[JsValue]) -> Result<JsValue, JsError>),
}

impl JsValue {
    /// Convert to boolean
    pub fn to_boolean(&self) -> bool {
        match self {
            JsValue::Undefined | JsValue::Null => false,
            JsValue::Boolean(b) => *b,
            JsValue::Number(n) => *n != 0.0 && !n.is_nan(),
            JsValue::String(s) => !s.is_empty(),
            JsValue::Array(_) | JsValue::Object(_) | JsValue::Function(_) | JsValue::NativeFunction(_) => true,
        }
    }

    /// Convert to number
    pub fn to_number(&self) -> f64 {
        match self {
            JsValue::Undefined => f64::NAN,
            JsValue::Null => 0.0,
            JsValue::Boolean(b) => if *b { 1.0 } else { 0.0 },
            JsValue::Number(n) => *n,
            JsValue::String(s) => s.parse().unwrap_or(f64::NAN),
            _ => f64::NAN,
        }
    }

    /// Convert to string
    pub fn to_string(&self) -> String {
        match self {
            JsValue::Undefined => String::from("undefined"),
            JsValue::Null => String::from("null"),
            JsValue::Boolean(b) => alloc::format!("{}", b),
            JsValue::Number(n) => alloc::format!("{}", n),
            JsValue::String(s) => s.clone(),
            JsValue::Array(_) => String::from("[object Array]"),
            JsValue::Object(_) => String::from("[object Object]"),
            JsValue::Function(_) => String::from("[Function]"),
            JsValue::NativeFunction(_) => String::from("[Function: native]"),
        }
    }

    /// Get type name
    pub fn type_name(&self) -> String {
        String::from(match self {
            JsValue::Undefined => "undefined",
            JsValue::Null => "object",
            JsValue::Boolean(_) => "boolean",
            JsValue::Number(_) => "number",
            JsValue::String(_) => "string",
            JsValue::Array(_) => "object",
            JsValue::Object(_) => "object",
            JsValue::Function(_) | JsValue::NativeFunction(_) => "function",
        })
    }

    /// Loose equality
    pub fn equals(&self, other: &JsValue) -> bool {
        match (self, other) {
            (JsValue::Undefined, JsValue::Undefined) => true,
            (JsValue::Null, JsValue::Null) => true,
            (JsValue::Undefined, JsValue::Null) | (JsValue::Null, JsValue::Undefined) => true,
            (JsValue::Number(a), JsValue::Number(b)) => a == b,
            (JsValue::String(a), JsValue::String(b)) => a == b,
            (JsValue::Boolean(a), JsValue::Boolean(b)) => a == b,
            _ => false,
        }
    }

    /// Strict equality
    pub fn strict_equals(&self, other: &JsValue) -> bool {
        match (self, other) {
            (JsValue::Undefined, JsValue::Undefined) => true,
            (JsValue::Null, JsValue::Null) => true,
            (JsValue::Number(a), JsValue::Number(b)) => a == b,
            (JsValue::String(a), JsValue::String(b)) => a == b,
            (JsValue::Boolean(a), JsValue::Boolean(b)) => a == b,
            _ => false,
        }
    }
}

/// JavaScript object
#[derive(Debug, Clone)]
pub struct JsObject {
    pub properties: BTreeMap<String, JsValue>,
    pub prototype: Option<Box<JsObject>>,
}

impl JsObject {
    pub fn new() -> Self {
        Self {
            properties: BTreeMap::new(),
            prototype: None,
        }
    }
}

impl Default for JsObject {
    fn default() -> Self {
        Self::new()
    }
}

/// JavaScript function
#[derive(Debug, Clone)]
pub struct JsFunction {
    pub name: String,
    pub params: Vec<String>,
    pub body: Box<AstNode>,
}

/// JavaScript context (scope)
#[derive(Debug, Clone)]
pub struct JsContext {
    variables: BTreeMap<String, JsValue>,
    parent: Option<Box<JsContext>>,
}

impl JsContext {
    pub fn new() -> Self {
        Self {
            variables: BTreeMap::new(),
            parent: None,
        }
    }

    pub fn child(&self) -> Self {
        Self {
            variables: BTreeMap::new(),
            parent: Some(Box::new(self.clone())),
        }
    }

    pub fn get(&self, name: &str) -> Option<JsValue> {
        if let Some(val) = self.variables.get(name) {
            Some(val.clone())
        } else if let Some(parent) = &self.parent {
            parent.get(name)
        } else {
            None
        }
    }

    pub fn set(&mut self, name: &str, value: JsValue) {
        self.variables.insert(String::from(name), value);
    }
}

impl Default for JsContext {
    fn default() -> Self {
        Self::new()
    }
}

/// JavaScript error
#[derive(Debug, Clone)]
pub enum JsError {
    SyntaxError(String),
    TypeError(String),
    ReferenceError(String),
    RangeError(String),
    Return(JsValue),
}

/// Timeout/interval
#[derive(Debug, Clone)]
struct Timeout {
    id: u32,
    callback: JsFunction,
    fire_at: u64,
    is_interval: bool,
}

// Lexer

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    String(String),
    Identifier(String),
    Keyword(String),
    Operator(String),
    Punctuation(char),
    Eof,
}

struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn tokenize(&mut self) -> Result<Vec<Token>, JsError> {
        let mut tokens = Vec::new();

        while !self.eof() {
            self.skip_whitespace();

            if self.eof() {
                break;
            }

            let c = self.current_char();

            // Skip comments
            if c == '/' && self.peek_char() == '/' {
                self.skip_line_comment();
                continue;
            }
            if c == '/' && self.peek_char() == '*' {
                self.skip_block_comment();
                continue;
            }

            // Number
            if c.is_ascii_digit() || (c == '.' && self.peek_char().is_ascii_digit()) {
                tokens.push(self.read_number()?);
                continue;
            }

            // String
            if c == '"' || c == '\'' || c == '`' {
                tokens.push(self.read_string(c)?);
                continue;
            }

            // Identifier or keyword
            if c.is_alphabetic() || c == '_' || c == '$' {
                tokens.push(self.read_identifier());
                continue;
            }

            // Operators
            if "+-*/%=<>!&|^~?:.".contains(c) {
                tokens.push(self.read_operator());
                continue;
            }

            // Punctuation
            if "(){}[];,".contains(c) {
                tokens.push(Token::Punctuation(c));
                self.advance();
                continue;
            }

            return Err(JsError::SyntaxError(alloc::format!("Unexpected character: {}", c)));
        }

        tokens.push(Token::Eof);
        Ok(tokens)
    }

    fn current_char(&self) -> char {
        self.input[self.pos..].chars().next().unwrap_or('\0')
    }

    fn peek_char(&self) -> char {
        self.input[self.pos..].chars().nth(1).unwrap_or('\0')
    }

    fn advance(&mut self) {
        self.pos += self.current_char().len_utf8();
    }

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn skip_whitespace(&mut self) {
        while !self.eof() && self.current_char().is_whitespace() {
            self.advance();
        }
    }

    fn skip_line_comment(&mut self) {
        while !self.eof() && self.current_char() != '\n' {
            self.advance();
        }
    }

    fn skip_block_comment(&mut self) {
        self.advance(); // /
        self.advance(); // *
        while !self.eof() {
            if self.current_char() == '*' && self.peek_char() == '/' {
                self.advance();
                self.advance();
                break;
            }
            self.advance();
        }
    }

    fn read_number(&mut self) -> Result<Token, JsError> {
        let start = self.pos;

        while !self.eof() && (self.current_char().is_ascii_digit() || self.current_char() == '.') {
            self.advance();
        }

        // Handle exponential notation
        if self.current_char() == 'e' || self.current_char() == 'E' {
            self.advance();
            if self.current_char() == '+' || self.current_char() == '-' {
                self.advance();
            }
            while !self.eof() && self.current_char().is_ascii_digit() {
                self.advance();
            }
        }

        let num_str = &self.input[start..self.pos];
        let num: f64 = num_str.parse().map_err(|_| JsError::SyntaxError(String::from("Invalid number")))?;
        Ok(Token::Number(num))
    }

    fn read_string(&mut self, quote: char) -> Result<Token, JsError> {
        self.advance(); // Opening quote
        let mut s = String::new();

        while !self.eof() && self.current_char() != quote {
            if self.current_char() == '\\' {
                self.advance();
                match self.current_char() {
                    'n' => s.push('\n'),
                    't' => s.push('\t'),
                    'r' => s.push('\r'),
                    '\\' => s.push('\\'),
                    '"' => s.push('"'),
                    '\'' => s.push('\''),
                    _ => s.push(self.current_char()),
                }
            } else {
                s.push(self.current_char());
            }
            self.advance();
        }

        if self.eof() {
            return Err(JsError::SyntaxError(String::from("Unterminated string")));
        }

        self.advance(); // Closing quote
        Ok(Token::String(s))
    }

    fn read_identifier(&mut self) -> Token {
        let start = self.pos;

        while !self.eof() && (self.current_char().is_alphanumeric() || self.current_char() == '_' || self.current_char() == '$') {
            self.advance();
        }

        let id = &self.input[start..self.pos];

        // Check for keywords
        let keywords = ["var", "let", "const", "function", "return", "if", "else", "while", "for", "do", "break", "continue", "switch", "case", "default", "try", "catch", "finally", "throw", "new", "this", "true", "false", "null", "undefined", "typeof", "instanceof", "in", "of", "class", "extends", "super", "static", "get", "set", "async", "await", "yield", "import", "export", "from", "as"];

        if keywords.contains(&id) {
            Token::Keyword(String::from(id))
        } else {
            Token::Identifier(String::from(id))
        }
    }

    fn read_operator(&mut self) -> Token {
        let start = self.pos;

        // Multi-character operators
        let ops = ["===", "!==", "==", "!=", "<=", ">=", "&&", "||", "++", "--", "+=", "-=", "*=", "/=", "%=", "**", "=>", "?.", "??"];

        for op in ops {
            if self.input[self.pos..].starts_with(op) {
                self.pos += op.len();
                return Token::Operator(String::from(op));
            }
        }

        // Single character operator
        let op = self.current_char();
        self.advance();
        Token::Operator(String::from(op))
    }
}

// Parser

#[derive(Debug, Clone)]
enum AstNode {
    Program(Vec<Box<AstNode>>),
    Number(f64),
    String(String),
    Boolean(bool),
    Null,
    Undefined,
    Identifier(String),
    BinaryOp { op: String, left: Box<AstNode>, right: Box<AstNode> },
    UnaryOp { op: String, operand: Box<AstNode> },
    Assignment { target: Box<AstNode>, value: Box<AstNode> },
    VarDecl { name: String, init: Option<Box<AstNode>>, kind: String },
    FunctionDecl { name: String, params: Vec<String>, body: Box<AstNode> },
    FunctionExpr { params: Vec<String>, body: Box<AstNode> },
    Call { callee: Box<AstNode>, args: Vec<Box<AstNode>> },
    MemberAccess { object: Box<AstNode>, property: String },
    IndexAccess { object: Box<AstNode>, index: Box<AstNode> },
    If { condition: Box<AstNode>, then_branch: Box<AstNode>, else_branch: Option<Box<AstNode>> },
    While { condition: Box<AstNode>, body: Box<AstNode> },
    For { init: Option<Box<AstNode>>, condition: Option<Box<AstNode>>, update: Option<Box<AstNode>>, body: Box<AstNode> },
    Return { value: Option<Box<AstNode>> },
    Block(Vec<Box<AstNode>>),
    Array(Vec<Box<AstNode>>),
    Object(Vec<(String, Box<AstNode>)>),
    Ternary { condition: Box<AstNode>, then_expr: Box<AstNode>, else_expr: Box<AstNode> },
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn parse(&mut self) -> Result<AstNode, JsError> {
        let mut statements = Vec::new();

        while !self.eof() {
            statements.push(Box::new(self.parse_statement()?));
        }

        Ok(AstNode::Program(statements))
    }

    fn current(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> &Token {
        let tok = self.current().clone();
        self.pos += 1;
        self.tokens.get(self.pos - 1).unwrap_or(&Token::Eof)
    }

    fn eof(&self) -> bool {
        matches!(self.current(), Token::Eof)
    }

    fn expect_punct(&mut self, c: char) -> Result<(), JsError> {
        if self.current() == &Token::Punctuation(c) {
            self.advance();
            Ok(())
        } else {
            Err(JsError::SyntaxError(alloc::format!("Expected '{}'", c)))
        }
    }

    fn parse_statement(&mut self) -> Result<AstNode, JsError> {
        match self.current() {
            Token::Keyword(kw) if kw == "var" || kw == "let" || kw == "const" => {
                self.parse_var_decl()
            }
            Token::Keyword(kw) if kw == "function" => {
                self.parse_function_decl()
            }
            Token::Keyword(kw) if kw == "return" => {
                self.parse_return()
            }
            Token::Keyword(kw) if kw == "if" => {
                self.parse_if()
            }
            Token::Keyword(kw) if kw == "while" => {
                self.parse_while()
            }
            Token::Keyword(kw) if kw == "for" => {
                self.parse_for()
            }
            Token::Punctuation('{') => {
                self.parse_block()
            }
            _ => {
                let expr = self.parse_expression()?;
                if self.current() == &Token::Punctuation(';') {
                    self.advance();
                }
                Ok(expr)
            }
        }
    }

    fn parse_var_decl(&mut self) -> Result<AstNode, JsError> {
        let kind = match self.advance() {
            Token::Keyword(k) => k.clone(),
            _ => String::from("var"),
        };

        let name = match self.advance() {
            Token::Identifier(n) => n.clone(),
            _ => return Err(JsError::SyntaxError(String::from("Expected identifier"))),
        };

        let init = if self.current() == &Token::Operator(String::from("=")) {
            self.advance();
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };

        if self.current() == &Token::Punctuation(';') {
            self.advance();
        }

        Ok(AstNode::VarDecl { name, init, kind })
    }

    fn parse_function_decl(&mut self) -> Result<AstNode, JsError> {
        self.advance(); // function

        let name = match self.advance() {
            Token::Identifier(n) => n.clone(),
            _ => return Err(JsError::SyntaxError(String::from("Expected function name"))),
        };

        self.expect_punct('(')?;

        let mut params = Vec::new();
        while self.current() != &Token::Punctuation(')') {
            if let Token::Identifier(p) = self.advance() {
                params.push(p.clone());
            }
            if self.current() == &Token::Punctuation(',') {
                self.advance();
            }
        }

        self.expect_punct(')')?;

        let body = Box::new(self.parse_block()?);

        Ok(AstNode::FunctionDecl { name, params, body })
    }

    fn parse_return(&mut self) -> Result<AstNode, JsError> {
        self.advance(); // return

        let value = if self.current() != &Token::Punctuation(';') && !self.eof() {
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };

        if self.current() == &Token::Punctuation(';') {
            self.advance();
        }

        Ok(AstNode::Return { value })
    }

    fn parse_if(&mut self) -> Result<AstNode, JsError> {
        self.advance(); // if
        self.expect_punct('(')?;
        let condition = Box::new(self.parse_expression()?);
        self.expect_punct(')')?;

        let then_branch = Box::new(self.parse_statement()?);

        let else_branch = if self.current() == &Token::Keyword(String::from("else")) {
            self.advance();
            Some(Box::new(self.parse_statement()?))
        } else {
            None
        };

        Ok(AstNode::If { condition, then_branch, else_branch })
    }

    fn parse_while(&mut self) -> Result<AstNode, JsError> {
        self.advance(); // while
        self.expect_punct('(')?;
        let condition = Box::new(self.parse_expression()?);
        self.expect_punct(')')?;
        let body = Box::new(self.parse_statement()?);

        Ok(AstNode::While { condition, body })
    }

    fn parse_for(&mut self) -> Result<AstNode, JsError> {
        self.advance(); // for
        self.expect_punct('(')?;

        let init = if self.current() != &Token::Punctuation(';') {
            Some(Box::new(self.parse_statement()?))
        } else {
            self.advance();
            None
        };

        let condition = if self.current() != &Token::Punctuation(';') {
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };
        if self.current() == &Token::Punctuation(';') {
            self.advance();
        }

        let update = if self.current() != &Token::Punctuation(')') {
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };

        self.expect_punct(')')?;

        let body = Box::new(self.parse_statement()?);

        Ok(AstNode::For { init, condition, update, body })
    }

    fn parse_block(&mut self) -> Result<AstNode, JsError> {
        self.expect_punct('{')?;

        let mut statements = Vec::new();
        while self.current() != &Token::Punctuation('}') && !self.eof() {
            statements.push(Box::new(self.parse_statement()?));
        }

        self.expect_punct('}')?;

        Ok(AstNode::Block(statements))
    }

    fn parse_expression(&mut self) -> Result<AstNode, JsError> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<AstNode, JsError> {
        let left = self.parse_ternary()?;

        if self.current() == &Token::Operator(String::from("=")) {
            self.advance();
            let right = self.parse_assignment()?;
            return Ok(AstNode::Assignment {
                target: Box::new(left),
                value: Box::new(right),
            });
        }

        Ok(left)
    }

    fn parse_ternary(&mut self) -> Result<AstNode, JsError> {
        let condition = self.parse_logical_or()?;

        if self.current() == &Token::Operator(String::from("?")) {
            self.advance();
            let then_expr = self.parse_expression()?;
            if self.current() != &Token::Operator(String::from(":")) {
                return Err(JsError::SyntaxError(String::from("Expected ':' in ternary")));
            }
            self.advance();
            let else_expr = self.parse_expression()?;
            return Ok(AstNode::Ternary {
                condition: Box::new(condition),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            });
        }

        Ok(condition)
    }

    fn parse_logical_or(&mut self) -> Result<AstNode, JsError> {
        let mut left = self.parse_logical_and()?;

        while self.current() == &Token::Operator(String::from("||")) {
            let op = String::from("||");
            self.advance();
            let right = self.parse_logical_and()?;
            left = AstNode::BinaryOp { op, left: Box::new(left), right: Box::new(right) };
        }

        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<AstNode, JsError> {
        let mut left = self.parse_equality()?;

        while self.current() == &Token::Operator(String::from("&&")) {
            let op = String::from("&&");
            self.advance();
            let right = self.parse_equality()?;
            left = AstNode::BinaryOp { op, left: Box::new(left), right: Box::new(right) };
        }

        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<AstNode, JsError> {
        let mut left = self.parse_comparison()?;

        loop {
            let op = match self.current() {
                Token::Operator(op) if op == "==" || op == "!=" || op == "===" || op == "!==" => op.clone(),
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison()?;
            left = AstNode::BinaryOp { op, left: Box::new(left), right: Box::new(right) };
        }

        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<AstNode, JsError> {
        let mut left = self.parse_additive()?;

        loop {
            let op = match self.current() {
                Token::Operator(op) if op == "<" || op == ">" || op == "<=" || op == ">=" => op.clone(),
                _ => break,
            };
            self.advance();
            let right = self.parse_additive()?;
            left = AstNode::BinaryOp { op, left: Box::new(left), right: Box::new(right) };
        }

        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<AstNode, JsError> {
        let mut left = self.parse_multiplicative()?;

        loop {
            let op = match self.current() {
                Token::Operator(op) if op == "+" || op == "-" => op.clone(),
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            left = AstNode::BinaryOp { op, left: Box::new(left), right: Box::new(right) };
        }

        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<AstNode, JsError> {
        let mut left = self.parse_unary()?;

        loop {
            let op = match self.current() {
                Token::Operator(op) if op == "*" || op == "/" || op == "%" => op.clone(),
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = AstNode::BinaryOp { op, left: Box::new(left), right: Box::new(right) };
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<AstNode, JsError> {
        match self.current() {
            Token::Operator(op) if op == "!" || op == "-" || op == "+" => {
                let op = op.clone();
                self.advance();
                let operand = self.parse_unary()?;
                Ok(AstNode::UnaryOp { op, operand: Box::new(operand) })
            }
            Token::Keyword(kw) if kw == "typeof" => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(AstNode::UnaryOp { op: String::from("typeof"), operand: Box::new(operand) })
            }
            _ => self.parse_call(),
        }
    }

    fn parse_call(&mut self) -> Result<AstNode, JsError> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.current() == &Token::Punctuation('(') {
                self.advance();
                let mut args = Vec::new();
                while self.current() != &Token::Punctuation(')') {
                    args.push(Box::new(self.parse_expression()?));
                    if self.current() == &Token::Punctuation(',') {
                        self.advance();
                    }
                }
                self.expect_punct(')')?;
                expr = AstNode::Call { callee: Box::new(expr), args };
            } else if self.current() == &Token::Operator(String::from(".")) {
                self.advance();
                let property = match self.advance() {
                    Token::Identifier(p) => p.clone(),
                    _ => return Err(JsError::SyntaxError(String::from("Expected property name"))),
                };
                expr = AstNode::MemberAccess { object: Box::new(expr), property };
            } else if self.current() == &Token::Punctuation('[') {
                self.advance();
                let index = self.parse_expression()?;
                self.expect_punct(']')?;
                expr = AstNode::IndexAccess { object: Box::new(expr), index: Box::new(index) };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<AstNode, JsError> {
        match self.current().clone() {
            Token::Number(n) => {
                self.advance();
                Ok(AstNode::Number(n))
            }
            Token::String(s) => {
                self.advance();
                Ok(AstNode::String(s))
            }
            Token::Identifier(id) => {
                self.advance();
                Ok(AstNode::Identifier(id))
            }
            Token::Keyword(kw) if kw == "true" => {
                self.advance();
                Ok(AstNode::Boolean(true))
            }
            Token::Keyword(kw) if kw == "false" => {
                self.advance();
                Ok(AstNode::Boolean(false))
            }
            Token::Keyword(kw) if kw == "null" => {
                self.advance();
                Ok(AstNode::Null)
            }
            Token::Keyword(kw) if kw == "undefined" => {
                self.advance();
                Ok(AstNode::Undefined)
            }
            Token::Keyword(kw) if kw == "function" => {
                self.advance();
                self.expect_punct('(')?;
                let mut params = Vec::new();
                while self.current() != &Token::Punctuation(')') {
                    if let Token::Identifier(p) = self.advance() {
                        params.push(p.clone());
                    }
                    if self.current() == &Token::Punctuation(',') {
                        self.advance();
                    }
                }
                self.expect_punct(')')?;
                let body = Box::new(self.parse_block()?);
                Ok(AstNode::FunctionExpr { params, body })
            }
            Token::Punctuation('(') => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect_punct(')')?;
                Ok(expr)
            }
            Token::Punctuation('[') => {
                self.advance();
                let mut elements = Vec::new();
                while self.current() != &Token::Punctuation(']') {
                    elements.push(Box::new(self.parse_expression()?));
                    if self.current() == &Token::Punctuation(',') {
                        self.advance();
                    }
                }
                self.expect_punct(']')?;
                Ok(AstNode::Array(elements))
            }
            Token::Punctuation('{') => {
                self.advance();
                let mut props = Vec::new();
                while self.current() != &Token::Punctuation('}') {
                    let key = match self.advance() {
                        Token::Identifier(k) => k.clone(),
                        Token::String(k) => k.clone(),
                        _ => return Err(JsError::SyntaxError(String::from("Expected property key"))),
                    };
                    if self.current() != &Token::Operator(String::from(":")) {
                        return Err(JsError::SyntaxError(String::from("Expected ':'")));
                    }
                    self.advance();
                    let val = self.parse_expression()?;
                    props.push((key, Box::new(val)));
                    if self.current() == &Token::Punctuation(',') {
                        self.advance();
                    }
                }
                self.expect_punct('}')?;
                Ok(AstNode::Object(props))
            }
            _ => Err(JsError::SyntaxError(String::from("Unexpected token"))),
        }
    }
}

// Native functions

fn native_console_log(args: &[JsValue]) -> Result<JsValue, JsError> {
    let msg: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    crate::kprintln!("[JS Console] {}", msg.join(" "));
    Ok(JsValue::Undefined)
}

fn native_math_abs(args: &[JsValue]) -> Result<JsValue, JsError> {
    let n = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    Ok(JsValue::Number(n.abs()))
}

fn native_math_floor(args: &[JsValue]) -> Result<JsValue, JsError> {
    let n = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    Ok(JsValue::Number(floor_f64(n)))
}

fn native_math_ceil(args: &[JsValue]) -> Result<JsValue, JsError> {
    let n = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    Ok(JsValue::Number(ceil_f64(n)))
}

fn native_math_round(args: &[JsValue]) -> Result<JsValue, JsError> {
    let n = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    Ok(JsValue::Number(round_f64(n)))
}

fn native_math_max(args: &[JsValue]) -> Result<JsValue, JsError> {
    let mut max = f64::NEG_INFINITY;
    for arg in args {
        let n = arg.to_number();
        if n > max {
            max = n;
        }
    }
    Ok(JsValue::Number(max))
}

fn native_math_min(args: &[JsValue]) -> Result<JsValue, JsError> {
    let mut min = f64::INFINITY;
    for arg in args {
        let n = arg.to_number();
        if n < min {
            min = n;
        }
    }
    Ok(JsValue::Number(min))
}

fn native_math_random(_args: &[JsValue]) -> Result<JsValue, JsError> {
    // Simple pseudo-random using time
    let time = crate::time::uptime_secs();
    let random = ((time * 1103515245 + 12345) % (1 << 31)) as f64 / (1u64 << 31) as f64;
    Ok(JsValue::Number(random))
}

fn native_parse_int(args: &[JsValue]) -> Result<JsValue, JsError> {
    let s = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let radix = args.get(1).map(|v| v.to_number() as i32).unwrap_or(10);
    let n = i64::from_str_radix(s.trim(), radix as u32).ok().map(|n| n as f64).unwrap_or(f64::NAN);
    Ok(JsValue::Number(n))
}

fn native_parse_float(args: &[JsValue]) -> Result<JsValue, JsError> {
    let s = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let n: f64 = s.trim().parse().unwrap_or(f64::NAN);
    Ok(JsValue::Number(n))
}

fn native_is_nan(args: &[JsValue]) -> Result<JsValue, JsError> {
    let n = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    Ok(JsValue::Boolean(n.is_nan()))
}

fn native_is_finite(args: &[JsValue]) -> Result<JsValue, JsError> {
    let n = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    Ok(JsValue::Boolean(n.is_finite()))
}

fn native_json_parse(args: &[JsValue]) -> Result<JsValue, JsError> {
    // Simple JSON parser (just handles basic cases)
    let s = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let s = s.trim();

    if s == "null" {
        return Ok(JsValue::Null);
    }
    if s == "true" {
        return Ok(JsValue::Boolean(true));
    }
    if s == "false" {
        return Ok(JsValue::Boolean(false));
    }
    if let Ok(n) = s.parse::<f64>() {
        return Ok(JsValue::Number(n));
    }
    if s.starts_with('"') && s.ends_with('"') {
        return Ok(JsValue::String(String::from(&s[1..s.len()-1])));
    }

    Err(JsError::SyntaxError(String::from("Invalid JSON")))
}

fn native_json_stringify(args: &[JsValue]) -> Result<JsValue, JsError> {
    let val = args.get(0).unwrap_or(&JsValue::Undefined);
    Ok(JsValue::String(stringify_value(val)))
}

fn stringify_value(val: &JsValue) -> String {
    match val {
        JsValue::Undefined => String::from("undefined"),
        JsValue::Null => String::from("null"),
        JsValue::Boolean(b) => alloc::format!("{}", b),
        JsValue::Number(n) => alloc::format!("{}", n),
        JsValue::String(s) => alloc::format!("\"{}\"", s),
        JsValue::Array(arr) => {
            let items: Vec<String> = arr.iter().map(stringify_value).collect();
            alloc::format!("[{}]", items.join(","))
        }
        JsValue::Object(obj) => {
            let items: Vec<String> = obj.properties.iter()
                .map(|(k, v)| alloc::format!("\"{}\":{}", k, stringify_value(v)))
                .collect();
            alloc::format!("{{{}}}", items.join(","))
        }
        _ => String::from("null"),
    }
}
