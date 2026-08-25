; ModuleID = 'kai_module'
source_filename = "kai_module"

%KaiWallclock._ptr_ = type { i64, i64, ptr }

@kai.str = private unnamed_addr constant [5 x i8] c"hello"

define void @expire_check(ptr %t) {
entry:
  %t1 = alloca ptr, align 8
  store ptr %t, ptr %t1, align 8
  ret void
}

define i32 @main() {
entry:
  %w = alloca ptr, align 8
  %str = call ptr @kai_string_new(ptr @kai.str, i64 5)
  store ptr %str, ptr %w, align 8
  %tmp = load ptr, ptr %w, align 8
  call void @expire_check(ptr %tmp)
  %wallclock.hdr = load ptr, ptr %w, align 8
  %wallclock.payload.p = getelementptr inbounds nuw %KaiWallclock._ptr_, ptr %wallclock.hdr, i32 0, i32 2
  %wallclock.payload.v = load ptr, ptr %wallclock.payload.p, align 8
  call void @kai_release(ptr %wallclock.payload.v)
  call void @kai_release(ptr %wallclock.hdr)
  ret i32 0
}

declare ptr @kai_string_new(ptr, i64)

declare void @kai_release(ptr)
