; ModuleID = 'kai_module'
source_filename = "kai_module"

@kai.panic.msg = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.src.file = private unnamed_addr constant [8 x i8] c"<stdin>\00", align 1
@kai.panic.msg.1 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.panic.msg.2 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1

define i32 @main() {
entry:
  %total = alloca i32, align 4
  %base = alloca i32, align 4
  store i32 6, ptr %base, align 4
  %tmp = load i32, ptr %base, align 4
  %ovf = call { i32, i1 } @llvm.smul.with.overflow.i32(i32 %tmp, i32 7)
  %ovf.flag = extractvalue { i32, i1 } %ovf, 1
  br i1 %ovf.flag, label %panic, label %arith.ok

panic:                                            ; preds = %entry
  call void @kai_panic(ptr @kai.panic.msg, i64 16, ptr @kai.src.file, i64 3, i64 17)
  unreachable

arith.ok:                                         ; preds = %entry
  %mul = extractvalue { i32, i1 } %ovf, 0
  store i32 %mul, ptr %total, align 4
  %old = load i32, ptr %total, align 4
  %ovf1 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old, i32 1)
  %ovf.flag2 = extractvalue { i32, i1 } %ovf1, 1
  br i1 %ovf.flag2, label %panic3, label %arith.ok4

panic3:                                           ; preds = %arith.ok
  call void @kai_panic(ptr @kai.panic.msg.1, i64 16, ptr @kai.src.file, i64 4, i64 5)
  unreachable

arith.ok4:                                        ; preds = %arith.ok
  %add = extractvalue { i32, i1 } %ovf1, 0
  store i32 %add, ptr %total, align 4
  %tmp5 = load i32, ptr %total, align 4
  %ge = icmp sge i32 %tmp5, 42
  br i1 %ge, label %and.rhs, label %and.end

and.rhs:                                          ; preds = %arith.ok4
  %tmp6 = load i32, ptr %total, align 4
  %gt = icmp sgt i32 %tmp6, 43
  %not = xor i1 %gt, true
  br label %and.end

and.end:                                          ; preds = %and.rhs, %arith.ok4
  %and.result = phi i1 [ %ge, %arith.ok4 ], [ %not, %and.rhs ]
  br i1 %and.result, label %if.then, label %if.else

if.then:                                          ; preds = %and.end
  %tmp7 = load i32, ptr %total, align 4
  %ovf8 = call { i32, i1 } @llvm.ssub.with.overflow.i32(i32 %tmp7, i32 1)
  %ovf.flag9 = extractvalue { i32, i1 } %ovf8, 1
  br i1 %ovf.flag9, label %panic10, label %arith.ok11

if.else:                                          ; preds = %and.end
  ret i32 0

if.end:                                           ; No predecessors!
  ret i32 0

panic10:                                          ; preds = %if.then
  call void @kai_panic(ptr @kai.panic.msg.2, i64 16, ptr @kai.src.file, i64 7, i64 16)
  unreachable

arith.ok11:                                       ; preds = %if.then
  %sub = extractvalue { i32, i1 } %ovf8, 0
  ret i32 %sub
}

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.smul.with.overflow.i32(i32, i32) #0

declare void @kai_panic(ptr, i64, ptr, i64, i64)

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.sadd.with.overflow.i32(i32, i32) #0

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.ssub.with.overflow.i32(i32, i32) #0

attributes #0 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
