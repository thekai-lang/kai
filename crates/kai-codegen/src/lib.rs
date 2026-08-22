use inkwell::context::Context;

pub fn emit_hello_llvm() -> String {
    let context = Context::create();
    let module = context.create_module("kai");
    let builder = context.create_builder();

    let i32_ty = context.i32_type();
    let fn_ty = i32_ty.fn_type(&[], false);
    let function = module.add_function("main", fn_ty, None);
    let entry = context.append_basic_block(function, "entry");

    builder.position_at_end(entry);
    builder.build_return(Some(&i32_ty.const_int(0, false))).unwrap();

    module.print_to_string().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_valid_llvm_ir() {
        let ir = emit_hello_llvm();
        assert!(ir.contains("define i32 @main"));
    }
}
