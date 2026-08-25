; ModuleID = 'kai_module'
source_filename = "kai_module"

@kai.str = private unnamed_addr constant [5 x i8] c"hello"

define ptr @produce(ptr %t) {
entry:
  %t1 = alloca ptr, align 8
  store ptr %t, ptr %t1, align 8
  %tmp = load ptr, ptr %t1, align 8
  call void @kai_retain(ptr %tmp)
  ret ptr %tmp
}

define i32 @consume(ptr %t) {
entry:
  %t1 = alloca ptr, align 8
  store ptr %t, ptr %t1, align 8
  ret i32 42
}

define void @maybe_escape(ptr %t) {
entry:
  %t1 = alloca ptr, align 8
  store ptr %t, ptr %t1, align 8
  ret void
}

define i32 @caller(ptr %t) {
entry:
  %t1 = alloca ptr, align 8
  store ptr %t, ptr %t1, align 8
  %tmp = load ptr, ptr %t1, align 8
  %call = call i32 @consume(ptr %tmp)
  ret i32 %call
}

define i32 @main() {
entry:
  %v = alloca i32, align 4
  %out = alloca ptr, align 8
  %tok = alloca ptr, align 8
  %str = call ptr @kai_string_new(ptr @kai.str, i64 5)
  store ptr %str, ptr %tok, align 8
  %tmp = load ptr, ptr %tok, align 8
  %call = call ptr @produce(ptr %tmp)
  store ptr %call, ptr %out, align 8
  %tmp1 = load ptr, ptr %out, align 8
  %call2 = call i32 @caller(ptr %tmp1)
  store i32 %call2, ptr %v, align 4
  %tmp3 = load i32, ptr %v, align 4
  ret i32 %tmp3
}

declare void @kai_retain(ptr)

declare ptr @kai_string_new(ptr, i64)
