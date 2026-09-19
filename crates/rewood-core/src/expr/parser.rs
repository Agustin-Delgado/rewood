use super::lexer::Token;
use super::{BinOp, Expr, ExprError, UnOp};

/// Pratt parser over the token stream. Precedence, lowest to highest:
/// `or` < `and` < comparisons < `+ -` < `* / %` < unary.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse(mut self) -> Result<Expr, ExprError> {
        let e = self.expr(0)?;
        if self.pos != self.tokens.len() {
            return Err(ExprError::Parse(format!(
                "token inesperado {:?} al final de la expresión",
                self.tokens[self.pos]
            )));
        }
        Ok(e)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn expect(&mut self, t: Token) -> Result<(), ExprError> {
        match self.next() {
            Some(ref got) if *got == t => Ok(()),
            got => Err(ExprError::Parse(format!(
                "se esperaba {t:?}, llegó {got:?}"
            ))),
        }
    }

    fn binding_power(t: &Token) -> Option<(u8, BinOp)> {
        Some(match t {
            Token::Or => (1, BinOp::Or),
            Token::And => (2, BinOp::And),
            Token::Lt => (3, BinOp::Lt),
            Token::Le => (3, BinOp::Le),
            Token::Gt => (3, BinOp::Gt),
            Token::Ge => (3, BinOp::Ge),
            Token::EqEq => (3, BinOp::Eq),
            Token::Ne => (3, BinOp::Ne),
            Token::Plus => (4, BinOp::Add),
            Token::Minus => (4, BinOp::Sub),
            Token::Star => (5, BinOp::Mul),
            Token::Slash => (5, BinOp::Div),
            Token::Percent => (5, BinOp::Mod),
            _ => return None,
        })
    }

    fn expr(&mut self, min_bp: u8) -> Result<Expr, ExprError> {
        let mut lhs = self.unary()?;
        while let Some(tok) = self.peek() {
            let Some((bp, op)) = Self::binding_power(tok) else {
                break;
            };
            if bp < min_bp {
                break;
            }
            self.next();
            let rhs = self.expr(bp + 1)?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Result<Expr, ExprError> {
        match self.peek() {
            Some(Token::Minus) => {
                self.next();
                Ok(Expr::Unary(UnOp::Neg, Box::new(self.unary()?)))
            }
            Some(Token::Not) => {
                self.next();
                Ok(Expr::Unary(UnOp::Not, Box::new(self.unary()?)))
            }
            _ => self.primary(),
        }
    }

    fn primary(&mut self) -> Result<Expr, ExprError> {
        match self.next() {
            Some(Token::Number(v)) => Ok(Expr::Number(v)),
            Some(Token::True) => Ok(Expr::Bool(true)),
            Some(Token::False) => Ok(Expr::Bool(false)),
            Some(Token::LParen) => {
                let e = self.expr(0)?;
                self.expect(Token::RParen)?;
                Ok(e)
            }
            Some(Token::Ident(name)) => {
                if self.peek() == Some(&Token::LParen) {
                    self.next();
                    let mut args = Vec::new();
                    if self.peek() != Some(&Token::RParen) {
                        loop {
                            args.push(self.expr(0)?);
                            if self.peek() == Some(&Token::Comma) {
                                self.next();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RParen)?;
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Ref(name))
                }
            }
            other => Err(ExprError::Parse(format!("token inesperado {other:?}"))),
        }
    }
}
