use ast::{Expr, ConstExpr, BlockExpr, LocalExpr, GetConstFieldExpr, GetFieldExpr, CallExpr, WhileLoopExpr, IfExpr, SwitchExpr, ObjectDeclExpr, ArrayDeclExpr, FunctionDeclExpr, NumOpExpr, BitOpExpr, ConstructExpr, ReturnExpr, ThrowExpr, AssignExpr};
use ast::{CNum, CInt, CString, CBool, CRegExp, CNull, CUndefined};
use ast::{OpSub, OpAdd, OpMul, OpDiv, OpMod};
use ast::{BitAnd, BitOr, BitXor, BitShl, BitShr};
use js::value::{Value, ValueData, VNull, VUndefined, VNumber, VInteger, VString, VObject, VBoolean, VFunction, ResultValue, to_value};
use js::object::ObjectData;
use js::function::{RegularFunc, RegularFunction};
use js::{console, math, object, array, function, json, number, error, uri};
use collections::treemap::TreeMap;
use std::vec::Vec;
use std::gc::Gc;
use std::cell::RefCell;
/// An execution engine
pub trait Executor {
	/// Makes a new execution engine
	fn new() -> ~Self;
	/// Sets a global variable
	fn set_global(&mut self, name:~str, val:Value) -> Value;
	/// Gets a global variable
	fn get_global(&self, name:~str) -> Value;
	/// Make a new scope
	fn make_scope(&mut self) -> Gc<RefCell<ObjectData>>;
	/// Destroy the current scope
	fn destroy_scope(&mut self) -> ();
	/// Runs the expression
	fn run(&mut self, expr:&Expr) -> ResultValue;
}
/// An intepreter
pub struct Interpreter {
	/// An object representing the global variables
	global: Value,
	/// The scopes
	scopes: Vec<Gc<RefCell<ObjectData>>>,
}
impl Executor for Interpreter {
	fn new() -> ~Interpreter {
		let global = ValueData::new_obj();
		global.borrow().set_field(~"console", console::_create());
		global.borrow().set_field(~"Math", math::_create());
		global.borrow().set_field(~"Object", object::_create());
		global.borrow().set_field(~"Array", array::_create());
		global.borrow().set_field(~"Function", function::_create());
		global.borrow().set_field(~"JSON", json::_create());
		global.borrow().set_field(~"Number", number::_create());
		global.borrow().set_field(~"Error", error::_create());
		number::init(global);
		uri::init(global);
		return ~Interpreter {global: global, scopes: Vec::new()};
	}
	fn set_global(&mut self, name:~str, val:Value) -> Value {
		self.global.borrow().set_field(name, val)
	}
	fn get_global(&self, name:~str) -> Value {
		self.global.borrow().get_field(name)
	}
	fn make_scope(&mut self) -> Gc<RefCell<ObjectData>> {
		let mut data = TreeMap::new();
		let value = Gc::new(RefCell::new(data));
		self.scopes.push(value.clone());
		value
	}
	fn destroy_scope(&mut self) -> () {
		self.scopes.pop();
	}
	fn run(&mut self, expr:&Expr) -> ResultValue {
		match *expr {
			ConstExpr(CNull) => Ok(Gc::new(VNull)),
			ConstExpr(CUndefined) => Ok(Gc::new(VUndefined)),
			ConstExpr(CNum(num)) => Ok(to_value(num)),
			ConstExpr(CInt(num)) => Ok(to_value(num as f64)),
			ConstExpr(CString(ref str)) => Ok(Gc::new(VString(str.to_owned()))),
			ConstExpr(CBool(val)) => Ok(Gc::new(VBoolean(val))),
			ConstExpr(CRegExp(ref reg, _, _)) => Ok(Gc::new(VBoolean(true))),
			BlockExpr(ref es) => {
				let mut obj = Gc::new(VNull);
				for e in es.iter() {
					let val = try!(self.run(*e));
					if e == es.last().unwrap() {
						obj = val;
					}
				}
				Ok(obj)
			},
			LocalExpr(ref name) => {
				let mut value = Gc::new(VUndefined);
				for scope in self.scopes.iter().rev() {
					match scope.borrow().borrow().find(name) {
						Some(v) => {
							value = v.value.clone();
							break;
						}
						None => ()
					}
				}
				Ok(if value.borrow() == &VUndefined {
					self.global.borrow().get_field(name.clone())
				} else {
					value
				})
			},
			GetConstFieldExpr(ref obj, ref field) => {
				let val_obj = try!(self.run(*obj));
				Ok(val_obj.borrow().get_field(field.clone()))
			},
			GetFieldExpr(ref obj, ref field) => {
				let val_obj = try!(self.run(*obj));
				let val_field = try!(self.run(*field));
				Ok(val_obj.borrow().get_field(val_field.borrow().to_str()))
			},
			CallExpr(ref callee, ref args) => {
				let (this, func) = match **callee {
					GetConstFieldExpr(ref obj, ref field) => {
						let obj = try!(self.run(*obj));
						(obj, obj.borrow().get_field(field.clone()))
					},
					GetFieldExpr(ref obj, ref field) => {
						let obj = try!(self.run(*obj));
						let field = try!(self.run(*field));
						(obj, obj.borrow().get_field(field.borrow().to_str()))
					},
					_ => (self.global.clone(), try!(self.run(callee.clone())))
				};
				let mut v_args = Vec::with_capacity(args.len());
				for arg in args.iter() {
					v_args.push(try!(self.run(*arg)));
				}
				match *func.borrow() {
					VFunction(ref func) => {
						func.borrow().call(self, this, Gc::new(VNull), v_args)
					},
					_ => Err(Gc::new(VUndefined))
				}
			},
			WhileLoopExpr(ref cond, ref expr) => {
				let mut result = Gc::new(VUndefined);
				while try!(self.run(*cond)).borrow().is_true() {
					result = try!(self.run(*expr));
				}
				Ok(result)
			},
			IfExpr(ref cond, ref expr, None) => {
				Ok(if try!(self.run(*cond)).borrow().is_true() {
					try!(self.run(*expr))
				} else {
					Gc::new(VUndefined)
				})
			},
			IfExpr(ref cond, ref expr, Some(ref else_e)) => {
				Ok(if try!(self.run(*cond)).borrow().is_true() {
					try!(self.run(*expr))
				} else {
					try!(self.run(*else_e))
				})
			},
			SwitchExpr(ref val_e, ref vals, ref default) => {
				let val = try!(self.run(*val_e)).borrow().clone();
				let mut result = Gc::new(VNull);
				let mut matched = false;
				for tup in vals.iter() {
					let tup:&(~Expr, Vec<~Expr>) = tup;
					match *tup {
						(ref cond, ref block) if(val == *try!(self.run(*cond)).borrow()) => {
							matched = true;
							let last_expr = block.last().unwrap();
							for expr in block.iter() {
								let e_result = try!(self.run(*expr));
								if expr == last_expr {
									result = e_result;
								}
							}
						},
						_ => ()
					}
				}
				if !matched && default.is_some() {
					result = try!(self.run(*default.as_ref().unwrap()));
				}
				Ok(result)
			},
			ObjectDeclExpr(ref map) => {
				let mut obj = ValueData::new_obj();
				for (key, val) in map.iter() {
					obj.borrow().set_field(key.clone(), try!(self.run(val.clone())));
				}
				obj.borrow().set_field(~"__proto__", self.get_global(~"Object").borrow().get_field(~"prototype"));
				Ok(obj)
			},
			ArrayDeclExpr(ref arr) => {
				let mut arr_map = ValueData::new_obj();
				let mut index : i32 = 0;
				for val in arr.iter() {
					let val = try!(self.run(val.clone()));
					arr_map.borrow().set_field(index.to_str(), val);
					index += 1;
				}
				arr_map.borrow().set_field(~"__proto__", self.get_global(~"Array").borrow().get_field(~"prototype"));
				arr_map.borrow().set_field(~"length", to_value(index));
				Ok(arr_map)
			},
			FunctionDeclExpr(ref name, ref args, ref expr) => {
				let function = RegularFunc(RegularFunction::new(*expr.clone(), args.clone()));
				let val = Gc::new(VFunction(RefCell::new(function)));
				if name.is_some() {
					self.global.borrow().set_field(name.clone().unwrap(), val);
				}
				Ok(val)
			},
			NumOpExpr(ref op, ref a, ref b) => {
				let v_a = try!(self.run(*a)).borrow().clone();
				let v_b = try!(self.run(*b)).borrow().clone();
				Ok(Gc::new(match *op {
					OpAdd => v_a + v_b,
					OpSub => v_a - v_b,
					OpMul => v_a * v_b,
					OpDiv => v_a / v_b,
					OpMod => v_a % v_b
				}))
			},
			BitOpExpr(ref op, ref a, ref b) => {
				let v_a = try!(self.run(*a)).borrow().clone();
				let v_b = try!(self.run(*b)).borrow().clone();
				Ok(Gc::new(match *op {
					BitAnd => v_a & v_b,
					BitOr => v_a | v_b,
					BitXor => v_a ^ v_b,
					BitShl => v_a << v_b,
					BitShr => v_a >> v_b
				}))
			},
			ConstructExpr(ref callee, ref args) => {
				let func = try!(self.run(callee.clone()));
				let mut v_args = Vec::with_capacity(args.len());
				for arg in args.iter() {
					v_args.push(try!(self.run(*arg)));
				}
				let this = Gc::new(VObject(RefCell::new(TreeMap::new())));
				this.borrow().set_field(~"__proto__", func.borrow().get_field(~"prototype"));
				Ok(match *func.borrow() {
					VFunction(ref func) => {
						func.borrow().call(self, this, Gc::new(VNull), v_args).unwrap();
						this
					},
					_ => Gc::new(VUndefined)
				})
			},
			ReturnExpr(ref ret) => {
				match *ret {
					Some(ref v) =>
						self.run(v.clone()),
					None => Ok(Gc::new(VUndefined))
				}
			},
			ThrowExpr(ref ex) => Err(try!(self.run(*ex))),
			AssignExpr(ref ref_e, ref val_e) => {
				let val = try!(self.run(*val_e));
				match **ref_e {
					LocalExpr(ref name) => {
						self.global.borrow().set_field(name.clone(), val);
					},
					GetConstFieldExpr(ref obj, ref field) => {
						let val_obj = try!(self.run(*obj));
						val_obj.borrow().set_field(field.clone(), val);
					},
					_ => ()
				}
				Ok(val)
			}
		}
	}
}