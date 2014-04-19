use ast::{TIdent, TNumber, TString, TSemicolon, TComment, TColon, TDot, TEqual, TOpenParen, TCloseParen, TComma, TOpenBlock, TCloseBlock, TOpenArray, TCloseArray, TQuestion, TNumOp, TBitOp, TCompOp, TLogOp, TArrow};
use ast::{OpAdd, OpSub, OpMul, OpDiv, OpMod};
use ast::{BitAnd, BitOr, BitXor};
use ast::{CompEqual, CompNotEqual, CompLessThan, CompGreaterThan, CompLessThanOrEqual, CompGreaterThanOrEqual};
use ast::{LogAnd, LogOr};
use ast::{Token, TokenData};
use std::io::{BufReader, BufferedReader, Buffer, IoResult, EndOfFile};
use std::strbuf::StrBuf;
use std::char::{from_u32, is_whitespace};
use std::num::from_str_radix;
#[deriving(Clone)]
#[deriving(Eq)]
#[deriving(Show)]
/// The type of string used
pub enum StringType {
	/// Double-quoted
	DoubleQuote,
	/// Single-quoted
	SingleQuote
}
#[deriving(Clone)]
#[deriving(Eq)]
#[deriving(Show)]
/// The type of comment used
pub enum CommentType {
	/// Multi-line comment
	MultiLineComment,
	/// Single-line comment
	SingleLineComment
}
#[deriving(Clone, Eq, Show)]
/// The type of number used
pub enum NumberType {
	/// Decimal number
	DecimalNumber,
	/// Hexadecimal number
	HexadecimalNumber,
	/// Octal number
	OctalNumber
}
/// The Javascript Lexer
pub struct Lexer<B> {
	/// The list of tokens generated so far
	pub tokens : Vec<Token>,
	/// The string buffer for identities
	ident_buffer : StrBuf,
	/// The string buffer for strings
	string_buffer : StrBuf,
	/// The string buffer for comments
	comment_buffer : StrBuf,
	/// The string buffer for numbers
	num_buffer : StrBuf,
	/// The kind of string - i.e. double quote or single quote or none if it isn't in a string
	string_start : Option<StringType>,
	/// The kind of comment
	current_comment : Option<CommentType>,
	/// The kind of number
	current_number : Option<NumberType>,
	/// If a backwards slash has just been read
	escaped : bool,
	/// The current line number
	line_number : uint,
	/// The current column number
	column_number : uint,
	/// The reader
	buffer: B,
	/// The current character
	current_char: Option<char>
}
impl<B:Buffer> Lexer<B> {
	/// Creates a new lexer with empty buffers
	pub fn new(buffer: B) -> Lexer<B> {
		return Lexer {
			tokens: Vec::new(),
			ident_buffer: StrBuf::with_capacity(32),
			string_buffer: StrBuf::with_capacity(256),
			num_buffer: StrBuf::with_capacity(16),
			comment_buffer: StrBuf::with_capacity(32),
			string_start: None,
			current_comment: None,
			current_number: None,
			escaped: false,
			line_number: 1,
			column_number: 0,
			buffer: buffer,
			current_char: None
		};
	}
	fn clear_buffer(&mut self) {
		if self.ident_buffer.len() > 0 {
			let ident = self.ident_buffer.clone().into_owned();
			self.push_token(TIdent(ident));
			self.ident_buffer.truncate(0);
		}
		if self.current_number.is_some() {
			let radix = match self.current_number.unwrap() {
				HexadecimalNumber => 16,
				OctalNumber => 8,
				DecimalNumber => 10
			};
			let num = match from_str_radix(self.num_buffer.as_slice(), radix) {
				Some(v) => v,
				None => fail!("{}:{}: Could not parse '{}' as a base {} number", self.line_number, self.column_number, self.num_buffer, radix)
			};
			self.push_token(TNumber(num));
			self.num_buffer.truncate(0);
			self.current_number = None;
		}
	}
	fn push_token(&mut self, tk:TokenData) {
		self.tokens.push(Token::new(tk, self.line_number, self.column_number))
	}
	/// Processes an input stream from a string into an array of tokens
	pub fn lex_str(script:~str) -> Vec<Token> {
		let script_bytes:&[u8] = script.as_bytes();
		let reader = BufReader::new(script_bytes);
		let buf_reader = BufferedReader::new(reader);
		let mut lexer = Lexer::new(buf_reader);
		lexer.lex().unwrap();
		lexer.tokens
	}
	fn next(&mut self) -> IoResult<char> {
		match self.current_char {
			Some(c) => {
				self.current_char = None;
				Ok(c)
			}
			None => self.buffer.read_char()
		}
	}
	fn peek(&mut self) -> IoResult<char> {
		let ch = try!(self.buffer.read_char());
		self.current_char = Some(ch);
		Ok(ch)
	}
	/// Processes an input stream from a BufferedReader into an array of tokens
	pub fn lex(&mut self) -> IoResult<()> {
		loop {
			let ch = match self.next() {
				Ok(ch) => ch,
				Err(ref err) if err.kind == EndOfFile => break,
				Err(err) => return Err(err)
			};
			self.column_number += 1;
			match ch {
				_ if self.escaped => {
					self.escaped = false;
					if ch != '\n' {
						let escaped_ch = match ch {
							'n' => '\n',
							'r' => '\r',
							't' => '\t',
							'b' => '\x08',
							'f' => '\x0c',
							'0' => '\0',
							'x' => {
								let mut nums = StrBuf::with_capacity(2);
								for _ in range(0, 2) {
									nums.push_char(try!(self.next()).clone());
								}
								self.column_number += 2;
								let as_num = match from_str_radix(nums.as_slice(), 16) {
									Some(v) => v,
									None => 0
								};
								match from_u32(as_num) {
									Some(v) => v,
									None => fail!("{}:{}: {} is not a valid unicode scalar value", self.line_number, self.column_number, as_num)
								}
							},
							'u' => {
								let mut nums = StrBuf::new();
								for _ in range(0, 4) {
									nums.push_char(try!(self.next()));
								}
								self.column_number += 4;
								let as_num = match from_str_radix(nums.as_slice(), 16) {
									Some(v) => v,
									None => 0
								};
								match from_u32(as_num) {
									Some(v) => v,
									None => fail!("{}:{}: {} is not a valid unicode scalar value", self.line_number, self.column_number, as_num)
								}
							},
							'\'' if self.string_start == Some(SingleQuote) => '\'',
							'"' if self.string_start == Some(DoubleQuote) => '"',
							_ => fail!("{}:{}: Invalid escape `{}`", self.line_number, self.column_number, ch)
						};
						self.string_buffer.push_char(escaped_ch);
					}
				},
				'\n' if self.current_comment == Some(SingleLineComment) => {
					let comment = self.comment_buffer.clone().into_owned();
					self.push_token(TComment(comment));
					self.comment_buffer.truncate(0);
					self.current_comment = None;
				},
				'*' if self.current_comment == Some(MultiLineComment) && self.peek() == Ok('/') => {
					self.current_char = None;
					let comment = self.comment_buffer.clone().into_owned();
					self.push_token(TComment(comment));
					self.comment_buffer.truncate(0);
					self.current_comment = None;
				},
				_ if self.current_comment.is_some() => {
					self.comment_buffer.push_char(ch);
				},
				'"' if self.string_start == Some(DoubleQuote) => {
					self.string_start = None;
					let string = self.string_buffer.clone().into_owned();
					self.push_token(TString(string));
					self.string_buffer.truncate(0);
				},
				'\'' if self.string_start == Some(SingleQuote) => {
					self.string_start = None;
					let string = self.string_buffer.clone().into_owned();
					self.push_token(TString(string));
					self.string_buffer.truncate(0);
				},
				'\\' if self.string_start.is_some() => self.escaped = true,
				_ if self.string_start.is_some() => self.string_buffer.push_char(ch),
				'"' if self.string_start.is_none() => self.string_start = Some(DoubleQuote),
				'0' if self.peek() == Ok('x') => {
					self.current_char = None;
					self.current_number = Some(HexadecimalNumber);
				},
				'0' if self.ident_buffer.len() == 0 && self.current_number.is_none() => {
					self.num_buffer.push_char(ch);
					self.current_number = Some(OctalNumber);
				},
				'0'..'7' if self.current_number == Some(OctalNumber) => {
					self.num_buffer.push_char(ch)
				},
				'8' | '9' if self.current_number == Some(OctalNumber) => {
					self.num_buffer.push_char(ch);
					self.current_number = Some(DecimalNumber);
				},
				'0'.. '9' | 'A'.. 'F' | 'a' .. 'f' if self.current_number == Some(HexadecimalNumber) => {
					self.num_buffer.push_char(ch)
				},
				'0'.. '9' if self.ident_buffer.len() == 0 => {
					self.num_buffer.push_char(ch);
					self.current_number = Some(DecimalNumber);
				},
				'\'' if self.string_start.is_none() => self.string_start = Some(SingleQuote),
				'.' if self.current_number.is_some() && !self.num_buffer.as_slice().contains(".") => {
					self.num_buffer.push_char(ch);
					self.current_number = Some(DecimalNumber);
				},
				';' => {
					self.clear_buffer();
					self.push_token(TSemicolon);
				},
				':' => {
					self.clear_buffer();
					self.push_token(TColon);
				},
				'.' => {
					self.clear_buffer();
					self.push_token(TDot);
				},
				'(' => {
					self.clear_buffer();
					self.push_token(TOpenParen);
				},
				')' => {
					self.clear_buffer();
					self.push_token(TCloseParen);
				},
				',' => {
					self.clear_buffer();
					self.push_token(TComma);
				},
				'{' => {
					self.clear_buffer();
					self.push_token(TOpenBlock);
				},
				'}' => {
					self.clear_buffer();
					self.push_token(TCloseBlock);
				},
				'[' => {
					self.clear_buffer();
					self.push_token(TOpenArray);
				},
				']' => {
					self.clear_buffer();
					self.push_token(TCloseArray);
				},
				'?' => {
					self.clear_buffer();
					self.push_token(TQuestion);
				},
				'/' if self.peek() == Ok('/') => {
					self.current_char = None;
					self.current_comment = Some(SingleLineComment);
				},
				'/' if self.peek() == Ok('*') => {
					self.current_char = None;
					self.current_comment = Some(MultiLineComment);
				},
				'/' => {
					self.clear_buffer();
					self.push_token(TNumOp(OpDiv));
				},
				'*' => {
					self.clear_buffer();
					self.push_token(TNumOp(OpMul));
				},
				'+' => {
					self.clear_buffer();
					self.push_token(TNumOp(OpAdd));
				},
				'-' => {
					self.clear_buffer();
					self.push_token(TNumOp(OpSub));
				},
				'%' => {
					self.clear_buffer();
					self.push_token(TNumOp(OpMod));
				},
				'|' if self.peek() == Ok('|') => {
					self.current_char = None;
					self.clear_buffer();
					self.push_token(TLogOp(LogOr));
				},
				'|' => {
					self.clear_buffer();
					self.push_token(TBitOp(BitOr));
				},
				'&' if self.peek() == Ok('&') => {
					self.current_char = None;
					self.clear_buffer();
					self.push_token(TLogOp(LogAnd));
				},
				'&' => {
					self.clear_buffer();
					self.push_token(TBitOp(BitAnd));
				},
				'^' => {
					self.clear_buffer();
					self.push_token(TBitOp(BitXor));
				},
				'=' if self.peek() == Ok('>') => {
					self.current_char = None;
					self.clear_buffer();
					self.push_token(TArrow);
				},
				'=' if self.peek() == Ok('=') => {
					self.current_char = None;
					self.clear_buffer();
					self.push_token(TCompOp(CompEqual));
				},
				'=' => {
					self.clear_buffer();
					self.push_token(TEqual);
				},
				'<' if self.peek() == Ok('=') => {
					self.current_char = None;
					self.clear_buffer();
					self.push_token(TCompOp(CompLessThanOrEqual));
				},
				'<' => {
					self.clear_buffer();
					self.push_token(TCompOp(CompLessThan));
				},
				'>' if self.peek() == Ok('=') => {
					self.current_char = None;
					self.clear_buffer();
					self.push_token(TCompOp(CompGreaterThanOrEqual));
				},
				'>' => {
					self.clear_buffer();
					self.push_token(TCompOp(CompGreaterThan));
				},
				'!' if self.peek() == Ok('=') => {
					self.current_char = None;
					self.clear_buffer();
					self.push_token(TCompOp(CompNotEqual));
				},
				'\n' => {
					self.clear_buffer();
					self.line_number += 1;
					self.column_number = 0;
				},
				'\r' => {
					self.clear_buffer();
					self.column_number = 0;
				},
				_ if is_whitespace(ch) => self.clear_buffer(),
				_ => self.ident_buffer.push_char(ch)
			};
		}
		self.clear_buffer();
		Ok(())
	}
}