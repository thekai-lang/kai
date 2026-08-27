; ModuleID = 'kai_module'
source_filename = "kai_module"

%KaiWallclock._ptr_ = type { i64, i64, ptr, i64, ptr }

@kai.str = private unnamed_addr constant [5 x i8] c"hello"

define void @expire_check(ptr %t) {
entry:
  %t1 = alloca ptr, align 8
  store ptr %t, ptr %t1, align 8
  %wallclock.hdr = load ptr, ptr %t1, align 8
  call void @kai_retain(ptr %wallclock.hdr)
  %wallclock.hdr2 = load ptr, ptr %t1, align 8
  call void @kai_wallclock_release(ptr %wallclock.hdr2)
  ret void
}

define i32 @main() {
entry:
  %w = alloca ptr, align 8
  %str = call ptr @kai_string_new(ptr @kai.str, i64 5)
  %wall.now = call i64 @kai_wallclock_now()
  %wall.hdr = call ptr @kai_wallclock_new(i64 %wall.now, ptr @kai.dtor.wall_string, i64 ptrtoint (ptr getelementptr (ptr, ptr null, i32 1) to i64))
  %wall.payload.p = getelementptr inbounds nuw %KaiWallclock._ptr_, ptr %wall.hdr, i32 0, i32 4
  store ptr %str, ptr %wall.payload.p, align 8
  store ptr %wall.hdr, ptr %w, align 8
  %tmp = load ptr, ptr %w, align 8
  call void @expire_check(ptr %tmp)
  %wallclock.hdr = load ptr, ptr %w, align 8
  call void @kai_wallclock_release(ptr %wallclock.hdr)
  ret i32 0
}

declare void @kai_retain(ptr)

declare void @kai_wallclock_release(ptr)

declare ptr @kai_string_new(ptr, i64)

declare i64 @kai_wallclock_now()

define void @kai.dtor.wall_string(ptr %0) {
entry:
  %wall.payload.p = getelementptr inbounds nuw %KaiWallclock._ptr_, ptr %0, i32 0, i32 4
  %rel.hdr = load ptr, ptr %wall.payload.p, align 8
  call void @kai_release(ptr %rel.hdr)
  ret void
}

declare void @kai_release(ptr)

declare ptr @kai_wallclock_new(i64, ptr, i64)
