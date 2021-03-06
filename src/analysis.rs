//! The static analysis component of Elgin
//! Does fun stuff like type inference

use crate::ir::*;
use crate::types::Type;
use crate::errors::Span;

use std::collections::HashMap;

type Constraints = Vec<(Type, Type)>;

impl<'i> IRBuilder<'i> {
    pub fn analyze(&mut self) -> Option<()> {
        self.scopes.clear();
        let mut new_procs = Vec::new();
        let mut index = 0;
        while index < self.procs.len() {
            self.scopes.push(HashMap::new());
            let scope = self.scopes.last_mut().unwrap();
            for (i, arg_type) in self.procs[index].arg_types.iter().enumerate() {
                scope.insert(self.procs[index].args[i].clone(), arg_type.clone());
            }
            let proc = self.procs[index].clone();
            let mut constraints = self.gen_constraints(&proc)?;
            add_literal_constaints(&mut constraints, &mut self.procs);
            new_procs.push(self.solve_constraints(&proc, &constraints)?);
            index += 1;
        }
        self.procs = dbg!(new_procs);
        Some(())
    }

    fn gen_constraints(&mut self, proc: &IRProc) -> Option<Constraints> {
        use InstructionType::*;
        let mut constraints = Vec::new();
        let mut stack = vec![];
        for ins in &proc.body {
            match ins.contents.ins.clone() {
                Push(_) => {
                    stack.push(ins.contents.typ.clone());
                }
                Load(var) => {
                    stack.push(self.locate_var(&var)?);
                }
                Store(var) => {
                    let typ = stack.pop().unwrap();
                    self.add_constraint(&mut constraints, ins.contents.typ.clone(), typ);
                    self.add_constraint(&mut constraints, ins.contents.typ.clone(), self.locate_var(&var)?);
                }
                StoreIndexed(var) => {
                    let _index_type = stack.pop().unwrap();
                    let value_type = stack.pop().unwrap();
                    if let Type::Array(_, t) = self.locate_var(&var)? {
                        self.add_constraint(&mut constraints, *t, value_type);
                    }
                    // TODO what happens here?
                }
                Allocate(var) => {
                    let content_type = stack.pop().unwrap();
                    let var_type = ins.contents.typ.clone();
                    let scope_index = self.scopes.len() - 1;
                    self.scopes[scope_index].insert(var, var_type.clone());
                    self.add_constraint(&mut constraints, var_type, content_type);
                }
                Index => {
                    let _index_type = stack.pop().unwrap();
                    let object_type = stack.pop().unwrap();
                    if let Type::Array(_, t) = object_type {
                        stack.push(*t);
                    } else {
                        panic!();
                    }
                }

                Branch(_, _) => {
                    self.add_constraint(
                        &mut constraints,
                        stack.pop().unwrap(),
                        Type::Bool,
                    );
                }
                Jump(_) => (),
                Label(_) => (),

                Call(proc_name) => {
                    let proc = self.locate_proc(&proc_name)?.clone();
                    //let arg_count = proc.arg_types.len();
                    {
                        let args = &stack[stack.len() - proc.args.len()..];
                        for (i, arg) in args.iter().enumerate() {
                            self.add_constraint(&mut constraints, arg.clone(), proc.arg_types[i].clone());
                        }
                    }
                    stack.truncate(stack.len() - proc.args.len());
                    stack.push(proc.ret_type.clone());
                }
                Return => {
                    let type_to_return = stack.pop().unwrap();
                    //let ret_type = ins.typ.clone();
                    self.add_constraint(&mut constraints, type_to_return, proc.ret_type.clone());
                }

                Negate(_) => {
                    let t1 = stack.pop().unwrap();
                    self.add_constraint(&mut constraints, t1.clone(), ins.contents.typ.clone());
                }
                // TODO more specific constraints???
                Add(_) | Subtract(_) | Multiply(_) | IntDivide | Divide => {
                    let t1 = stack.pop().unwrap();
                    let t2 = stack.pop().unwrap();
                    self.add_constraint(&mut constraints, t1.clone(), t2.clone());
                    self.add_constraint(&mut constraints, t1.clone(), ins.contents.typ.clone());
                    self.add_constraint(&mut constraints, t2.clone(), ins.contents.typ.clone());
                    stack.push(ins.contents.typ.clone());
                }

                Compare(_) => {
                    let t1 = stack.pop().unwrap();
                    let t2 = stack.pop().unwrap();
                    self.add_constraint(&mut constraints, t1.clone(), t2.clone());
                    self.add_constraint(
                        &mut constraints,
                        ins.contents.typ.clone(),
                        Type::Bool,
                    );
                    stack.push(Type::Bool);
                }
            };
        }
        Some(constraints)
    }

    fn solve_constraints(&self, proc: &IRProc, constraints: &Constraints) -> Option<IRProc> {
        println!("Generated constraints:");
        for (t1, t2) in constraints {
            println!("{:?} == {:?}", t1, t2);
        }
        println!("------------------------");
        let mut new_body = proc.body.clone();
        let mut new_constraints = constraints.clone();

        //while new_constraints.len() > 0 {
        for _ in 1..4 {
            for (t1, t2) in constraints {
                // set t1 == t2
                new_body = substitute_proc_body(new_body, t1, t2); // replace in the proc
                new_constraints = substitute_constraints(&new_constraints, t1, t2);
                // replace in the rules
            }
        }

        Some(IRProc {
            name: proc.name.clone(),
            args: proc.args.clone(),
            arg_types: proc.arg_types.clone(),
            ret_type: proc.ret_type.clone(),
            body: new_body,
        })
    }


    fn add_constraint(&mut self, constraints: &mut Constraints, t1: Type, t2: Type) {
        println!("Trying to add constraint: {:?} == {:?}", t1.clone(), t2.clone());
        // TODO Some of these constraints just shouldn't be permitted at all and should raise a type
        // error. For example, you shouldn't be able to add a constraint i8 == f64
        if t1 == t2 {
            return;
        }
        if t1 == Type::StrLiteral || t2 == Type::StrLiteral {
            return;
        }
        if t1 == Type::Undefined || t2 == Type::Undefined {
            return;
        }
        println!("After transformation: {:?} == {:?}", t1.clone(), t2.clone());
        if let Type::Variable(_) = t2 {
            constraints.push((t2, t1));
        } else {
            if t2 == Type::IntLiteral
                || t2 == Type::FloatLiteral
                || t2 == Type::StrLiteral {
                constraints.push((t2, t1));
            } else {
                constraints.push((t1, t2));
            }
        }
    }
}

fn substitute_proc_body(body: Vec<Span<Instruction>>, t1: &Type, t2: &Type) -> Vec<Span<Instruction>> {
    let mut new_body = vec![];

    for ins in body {
        new_body.push(spanned(Instruction {
            ins: ins.contents.ins,
            typ: if ins.contents.typ.clone() == t1.clone() {
                t2.clone()
            //} else if ins.typ.clone() == t2.clone() {
            //    t1.clone()
            } else {
                ins.contents.typ
            },
        }, ins.pos, ins.len));
    }
    new_body
}

fn substitute_constraints(constraints: &Constraints, t1: &Type, t2: &Type) -> Constraints {
    let mut new_constraints = Vec::new();

    for (left, right) in constraints {
        let new_left = if *left == *t1 {
            t2.clone()
        } else {
            left.clone()
        };

        let new_right = if *right == *t1 {
            t2.clone()
        } else {
            right.clone()
        };

        new_constraints.push((new_left, new_right));
    }

    new_constraints
}

fn add_literal_constaints(constraints: &mut Constraints, procs: &mut Vec<IRProc>) {
    let mut has_int_literal = false;
    let mut has_float_literal = false;
    for proc in procs {
        for ins in &proc.body {
            if ins.contents.typ == Type::IntLiteral {
                has_int_literal = true;

            } else if ins.contents.typ == Type::FloatLiteral {
                has_float_literal = true;
            }
        }
    }

    if has_int_literal {
        constraints.push((Type::IntLiteral, Type::I32));
    }
    if has_float_literal {
        constraints.push((Type::FloatLiteral, Type::F64));
    }
}
