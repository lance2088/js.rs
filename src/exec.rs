use ast::*;
use js::value::{Value, VNull, VUndefined, VNumber, VString, VObject, VBoolean, VFunction, ResultValue};
use js::object::*;
use collections::treemap::TreeMap;
use std::vec::Vec;
use std::f64;
use std::gc::Gc;
use std::cell::{RefCell, Ref};
use js::{console, math, object, array, function, json};
/// An execution engine
pub trait Executor {
	/// Makes a new execution engine
	fn new() -> ~Self;
	/// Sets a global variable
	fn set_global(&mut self, name:~str, val:Value) -> ();
	/// Gets a global variable
	fn get_global(&self, name:~str) -> Value;
	/// Runs the expression
	fn run(&mut self, expr:&Expr) -> ResultValue;
}
/// An intepreter
pub struct Interpreter {
	/// A hash map representing the global variables
	globals: ObjectData,
	/// The function scopes currently in use
	function_scopes: Vec<ObjectData>,
	/// The current function scope
	current_scope: uint
}
impl Executor for Interpreter {
	fn new() -> ~Interpreter {
		let mut globals : ObjectData = TreeMap::new();
		globals.swap(~"NaN", Gc::new(VNumber(f64::NAN)));
		globals.swap(~"Infinity", Gc::new(VNumber(f64::INFINITY)));
		globals.swap(~"console", console::_create());
		globals.swap(~"Math", math::_create());
		globals.swap(~"Object", object::_create());
		globals.swap(~"Array", array::_create());
		globals.swap(~"Function", function::_create());
		globals.swap(~"JSON", json::_create());
		return ~Interpreter {globals: globals, function_scopes: Vec::new(), current_scope: 0};
	}
	fn set_global(&mut self, name:~str, val:Value) {
		self.globals.swap(name, val);
	}
	fn get_global(&self, name:~str) -> Value {
		match self.globals.find(&name) {
			None => Gc::new(VUndefined),
			Some(ref g) => **g
		}
	}
	fn run(&mut self, expr:&Expr) -> ResultValue {
		match *expr {
			ConstExpr(CNull) => Ok(Gc::new(VNull)),
			ConstExpr(CUndefined) => Ok(Gc::new(VUndefined)),
			ConstExpr(CNum(num)) => Ok(Gc::new(VNumber(num))),
			ConstExpr(CInt(num)) => Ok(Gc::new(VNumber(num as f64))),
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
			LocalExpr(ref name) => Ok(match self.globals.find(name) {
				None => Gc::new(VUndefined),
				Some(v) => v.clone()
			}),
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
				let func = try!(self.run(callee.clone()));
				let mut v_args = Vec::with_capacity(args.len());
				for arg in args.iter() {
					v_args.push(try!(self.run(*arg)));
				}
				Ok(match *func.borrow() {
					VFunction(ref func) => func.borrow().call(Gc::new(VObject(RefCell::new(self.globals.clone()))), Gc::new(VNull), v_args).unwrap(),
					_ => Gc::new(VUndefined)
				})
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
				let mut obj = TreeMap::new();
				for (key, val) in map.iter() {
					obj.insert(key.clone(), try!(self.run(val.clone())));
				}
				obj.swap(~"__proto__", self.globals.find(&~"Object").unwrap().clone());
				Ok(Gc::new(VObject(RefCell::new(obj))))
			},
			ArrayDeclExpr(ref arr) => {
				let mut arr_map = TreeMap::new();
				let mut index = 0;
				for val in arr.iter() {
					let val = try!(self.run(val.clone()));
					arr_map.insert(index.to_str(), val);
					index += 1;
				}
				arr_map.swap(~"__proto__", self.globals.find(&~"Array").unwrap().clone());
				Ok(Gc::new(VObject(RefCell::new(arr_map))))
			},
			FunctionDeclExpr(ref name, ref args, ref expr) => {
				println!("Name: {}, args: {}, expr: {}", name, args, expr);
				Ok(Gc::new(VNull))
			},
			NumOpExpr(ref op, ref a, ref b) => {
				let v_a = try!(self.run(*a)).borrow().clone();
				let v_b = try!(self.run(*b)).borrow().clone();
				Ok(Gc::new(match *op {
					OpAdd => v_a + v_b,
					OpSub => v_a - v_b,
					OpMul => v_a * v_b,
					OpDiv => v_a / v_b,
					OpAnd => v_a & v_b,
					OpOr => v_a | v_b,
				}))
			},
			ConstructExpr(ref func, ref args) => {
				Ok(Gc::new(VNull))
			},
			ReturnExpr(ref ret) => {
				let v_ret = try!(self.run(ret.clone().unwrap()));
				Ok(Gc::new(VNull))
			},
			ThrowExpr(ref ex) => Err(try!(self.run(*ex))),
			AssignExpr(ref ref_e, ref val_e) => {
				let val = try!(self.run(*val_e));
				match **ref_e {
					LocalExpr(ref name) => {
						self.globals.insert(name.clone(), val);
					},
					_ => ()
				}
				Ok(val)
			}
		}
	}
}