; ModuleID = 'kai_module'
source_filename = "kai_module"

%KaiClosure = type { ptr, ptr }
%KaiEnvCaps.0 = type { i32, ptr }
%"KaiArray.\22%KaiEnvCaps.0 = type { i32, ptr }\22" = type { i64, i64, i64, ptr, ptr }

@kai.panic.msg = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.src.file = private unnamed_addr constant [9 x i8] c"main.kai\00", align 1
@kai.str = private unnamed_addr constant [1 x i8] c"x"
@kai.panic.msg.1 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.panic.msg.2 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1

define %KaiClosure @make_adder(i32 %base) {
entry:
  %base1 = alloca i32, align 4
  store i32 %base, ptr %base1, align 4
  %env.hdr = call ptr @kai_array_new(i64 1, i64 ptrtoint (ptr getelementptr (%KaiEnvCaps.0, ptr null, i32 1) to i64), ptr null)
  %env.payload.p = getelementptr inbounds nuw %"KaiArray.\22%KaiEnvCaps.0 = type { i32, ptr }\22", ptr %env.hdr, i32 0, i32 3
  %env.payload = load ptr, ptr %env.payload.p, align 8
  %cap.dst = getelementptr inbounds nuw %KaiEnvCaps.0, ptr %env.payload, i32 0, i32 0
  %cap.v = load i32, ptr %base1, align 4
  store i32 %cap.v, ptr %cap.dst, align 4
  %clo.e = insertvalue %KaiClosure { ptr @kai.clo.0, ptr undef }, ptr %env.hdr, 1
  ret %KaiClosure %clo.e
}

define { i64, i32 } @nothing() {
entry:
  ret { i64, i32 } { i64 1, i32 0 }
}

define i32 @loud_fallback() {
entry:
  ret i32 100
}

define i32 @main() {
entry:
  %"$tmp" = alloca ptr, align 8
  %f = alloca { i64, ptr }, align 8
  %e = alloca i32, align 4
  %add = alloca %KaiClosure, align 8
  %d = alloca i32, align 4
  %co.r13 = alloca i32, align 4
  %c = alloca i32, align 4
  %co.r4 = alloca i32, align 4
  %raw = alloca { i64, i32 }, align 8
  %b = alloca i32, align 4
  %co.r = alloca i32, align 4
  %a = alloca { i64, i32 }, align 8
  store { i64, i32 } { i64 0, i32 10 }, ptr %a, align 4
  %tmp = load { i64, i32 }, ptr %a, align 4
  %tag = extractvalue { i64, i32 } %tmp, 0
  %active = icmp eq i64 %tag, 0
  br i1 %active, label %co.some, label %co.fallback

co.some:                                          ; preds = %entry
  %payload = extractvalue { i64, i32 } %tmp, 1
  store i32 %payload, ptr %co.r, align 4
  br label %co.join

co.fallback:                                      ; preds = %entry
  store i32 0, ptr %co.r, align 4
  br label %co.join

co.join:                                          ; preds = %co.fallback, %co.some
  %co.v = load i32, ptr %co.r, align 4
  store i32 %co.v, ptr %b, align 4
  %call = call { i64, i32 } @nothing()
  store { i64, i32 } %call, ptr %raw, align 4
  %tmp1 = load { i64, i32 }, ptr %raw, align 4
  %tag2 = extractvalue { i64, i32 } %tmp1, 0
  %active3 = icmp eq i64 %tag2, 0
  br i1 %active3, label %co.some5, label %co.fallback6

co.some5:                                         ; preds = %co.join
  %payload8 = extractvalue { i64, i32 } %tmp1, 1
  store i32 %payload8, ptr %co.r4, align 4
  br label %co.join7

co.fallback6:                                     ; preds = %co.join
  store i32 5, ptr %co.r4, align 4
  br label %co.join7

co.join7:                                         ; preds = %co.fallback6, %co.some5
  %co.v9 = load i32, ptr %co.r4, align 4
  store i32 %co.v9, ptr %c, align 4
  %tmp10 = load { i64, i32 }, ptr %raw, align 4
  %tag11 = extractvalue { i64, i32 } %tmp10, 0
  %active12 = icmp eq i64 %tag11, 0
  br i1 %active12, label %co.some14, label %co.fallback15

co.some14:                                        ; preds = %co.join7
  %payload17 = extractvalue { i64, i32 } %tmp10, 1
  store i32 %payload17, ptr %co.r13, align 4
  br label %co.join16

co.fallback15:                                    ; preds = %co.join7
  store i32 2, ptr %co.r13, align 4
  br label %co.join16

co.join16:                                        ; preds = %co.fallback15, %co.some14
  %co.v18 = load i32, ptr %co.r13, align 4
  store i32 %co.v18, ptr %d, align 4
  %call19 = call %KaiClosure @make_adder(i32 20)
  store %KaiClosure %call19, ptr %add, align 8
  %tmp20 = load %KaiClosure, ptr %add, align 8
  %clo.code = extractvalue %KaiClosure %tmp20, 0
  %clo.env = extractvalue %KaiClosure %tmp20, 1
  %tmp21 = load i32, ptr %d, align 4
  %icall = call i32 %clo.code(ptr %clo.env, i32 %tmp21)
  store i32 %icall, ptr %e, align 4
  store { i64, ptr } { i64 1, ptr null }, ptr %f, align 8
  %str = call ptr @kai_string_new(ptr @kai.str, i64 1)
  store ptr %str, ptr %"$tmp", align 8
  %tmp22 = load ptr, ptr %"$tmp", align 8
  call void @kai_retain(ptr %tmp22)
  %some.payload = insertvalue { i64, ptr } { i64 0, ptr undef }, ptr %tmp22, 1
  call void @kai.release_string_(ptr %f)
  store { i64, ptr } %some.payload, ptr %f, align 8
  %tmp23 = load { i64, ptr }, ptr %f, align 8
  %tmp24 = load i32, ptr %b, align 4
  %tmp25 = load i32, ptr %e, align 4
  %ovf = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %tmp24, i32 %tmp25)
  %ovf.flag = extractvalue { i32, i1 } %ovf, 1
  br i1 %ovf.flag, label %panic, label %arith.ok

panic:                                            ; preds = %co.join16
  call void @kai_panic(ptr @kai.panic.msg.1, i64 16, ptr @kai.src.file, i64 33, i64 12)
  unreachable

arith.ok:                                         ; preds = %co.join16
  %add26 = extractvalue { i32, i1 } %ovf, 0
  %ovf27 = call { i32, i1 } @llvm.ssub.with.overflow.i32(i32 %add26, i32 30)
  %ovf.flag28 = extractvalue { i32, i1 } %ovf27, 1
  br i1 %ovf.flag28, label %panic29, label %arith.ok30

panic29:                                          ; preds = %arith.ok
  call void @kai_panic(ptr @kai.panic.msg.2, i64 16, ptr @kai.src.file, i64 33, i64 12)
  unreachable

arith.ok30:                                       ; preds = %arith.ok
  %sub = extractvalue { i32, i1 } %ovf27, 0
  %rel.hdr = load ptr, ptr %"$tmp", align 8
  call void @kai_release(ptr %rel.hdr)
  call void @kai.release_string_(ptr %f)
  %clo.env.p = getelementptr inbounds nuw %KaiClosure, ptr %add, i32 0, i32 1
  %clo.env31 = load ptr, ptr %clo.env.p, align 8
  call void @kai_release(ptr %clo.env31)
  ret i32 %sub
}

declare ptr @kai_array_new(i64, i64, ptr)

define i32 @kai.clo.0(ptr %0, i32 %1) {
entry:
  %p0 = alloca i32, align 4
  store i32 %1, ptr %p0, align 4
  %env.payload.p = getelementptr inbounds nuw %"KaiArray.\22%KaiEnvCaps.0 = type { i32, ptr }\22", ptr %0, i32 0, i32 3
  %env.payload = load ptr, ptr %env.payload.p, align 8
  %cap.view = getelementptr inbounds nuw %KaiEnvCaps.0, ptr %env.payload, i32 0, i32 0
  %tmp = load i32, ptr %cap.view, align 4
  %tmp1 = load i32, ptr %p0, align 4
  %ovf = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %tmp, i32 %tmp1)
  %ovf.flag = extractvalue { i32, i1 } %ovf, 1
  br i1 %ovf.flag, label %panic, label %arith.ok

panic:                                            ; preds = %entry
  call void @kai_panic(ptr @kai.panic.msg, i64 16, ptr @kai.src.file, i64 11, i64 43)
  unreachable

arith.ok:                                         ; preds = %entry
  %add = extractvalue { i32, i1 } %ovf, 0
  ret i32 %add
}

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.sadd.with.overflow.i32(i32, i32) #0

declare void @kai_panic(ptr, i64, ptr, i64, i64)

declare ptr @kai_string_new(ptr, i64)

declare void @kai_retain(ptr)

define void @kai.release_string_(ptr %0) {
entry:
  %tag.p = getelementptr inbounds nuw { i64, ptr }, ptr %0, i32 0, i32 0
  %tag = load i64, ptr %tag.p, align 4
  %active = icmp eq i64 %tag, 0
  br i1 %active, label %active.payload, label %inactive

inactive:                                         ; preds = %active.payload, %entry
  ret void

active.payload:                                   ; preds = %entry
  %payload.p = getelementptr inbounds nuw { i64, ptr }, ptr %0, i32 0, i32 1
  %rel.hdr = load ptr, ptr %payload.p, align 8
  call void @kai_release(ptr %rel.hdr)
  br label %inactive
}

declare void @kai_release(ptr)

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.ssub.with.overflow.i32(i32, i32) #0

attributes #0 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
