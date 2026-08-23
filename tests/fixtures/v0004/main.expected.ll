; ModuleID = 'kai_module'
source_filename = "kai_module"

%math.geometry.Point = type { i32, i32 }

@kai.panic.msg = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.src.file = private unnamed_addr constant [9 x i8] c"main.kai\00", align 1
@kai.panic.msg.1 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.panic.msg.2 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.panic.msg.3 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.src.file.4 = private unnamed_addr constant [18 x i8] c"math/geometry.kai\00", align 1
@kai.panic.msg.5 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.panic.msg.6 = private unnamed_addr constant [17 x i8] c"division by zero\00", align 1
@kai.panic.msg.7 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.panic.msg.8 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.src.file.9 = private unnamed_addr constant [15 x i8] c"util/flags.kai\00", align 1

define i32 @main() {
entry:
  %flag = alloca i32, align 4
  %doubled = alloca %math.geometry.Point, align 8
  %origin = alloca %math.geometry.Point, align 8
  %tmp = alloca %math.geometry.Point, align 8
  %f = getelementptr inbounds nuw %math.geometry.Point, ptr %tmp, i32 0, i32 0
  store i32 3, ptr %f, align 4
  %f1 = getelementptr inbounds nuw %math.geometry.Point, ptr %tmp, i32 0, i32 1
  store i32 4, ptr %f1, align 4
  %lit = load %math.geometry.Point, ptr %tmp, align 4
  store %math.geometry.Point %lit, ptr %origin, align 4
  %tmp2 = load %math.geometry.Point, ptr %origin, align 4
  %call = call %math.geometry.Point @math.geometry.double(%math.geometry.Point %tmp2)
  store %math.geometry.Point %call, ptr %doubled, align 4
  %field = getelementptr inbounds nuw %math.geometry.Point, ptr %doubled, i32 0, i32 0
  %field3 = load i32, ptr %field, align 4
  %call4 = call i32 @util.flags.pick(i32 %field3)
  store i32 %call4, ptr %flag, align 4
  %field5 = getelementptr inbounds nuw %math.geometry.Point, ptr %doubled, i32 0, i32 0
  %field6 = load i32, ptr %field5, align 4
  %eq = icmp eq i32 %field6, 6
  br i1 %eq, label %and.rhs, label %and.end

and.rhs:                                          ; preds = %entry
  %field7 = getelementptr inbounds nuw %math.geometry.Point, ptr %doubled, i32 0, i32 1
  %field8 = load i32, ptr %field7, align 4
  %eq9 = icmp eq i32 %field8, 8
  br label %and.end

and.end:                                          ; preds = %and.rhs, %entry
  %and.result = phi i1 [ %eq, %entry ], [ %eq9, %and.rhs ]
  br i1 %and.result, label %and.rhs10, label %and.end11

and.rhs10:                                        ; preds = %and.end
  %tmp12 = load i32, ptr %flag, align 4
  %eq13 = icmp eq i32 %tmp12, 7
  br label %and.end11

and.end11:                                        ; preds = %and.rhs10, %and.end
  %and.result14 = phi i1 [ %and.result, %and.end ], [ %eq13, %and.rhs10 ]
  br i1 %and.result14, label %and.rhs15, label %and.end16

and.rhs15:                                        ; preds = %and.end11
  %call17 = call i32 @math.geometry.tag()
  %eq18 = icmp eq i32 %call17, 10
  br label %and.end16

and.end16:                                        ; preds = %and.rhs15, %and.end11
  %and.result19 = phi i1 [ %and.result14, %and.end11 ], [ %eq18, %and.rhs15 ]
  br i1 %and.result19, label %if.then, label %if.end

if.then:                                          ; preds = %and.end16
  %field20 = getelementptr inbounds nuw %math.geometry.Point, ptr %doubled, i32 0, i32 0
  %field21 = load i32, ptr %field20, align 4
  %field22 = getelementptr inbounds nuw %math.geometry.Point, ptr %doubled, i32 0, i32 1
  %field23 = load i32, ptr %field22, align 4
  %ovf = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %field21, i32 %field23)
  %ovf.flag = extractvalue { i32, i1 } %ovf, 1
  br i1 %ovf.flag, label %panic, label %arith.ok

if.end:                                           ; preds = %and.end16
  ret i32 0

panic:                                            ; preds = %if.then
  call void @kai_panic(ptr @kai.panic.msg, i64 16, ptr @kai.src.file, i64 20, i64 16)
  unreachable

arith.ok:                                         ; preds = %if.then
  %add = extractvalue { i32, i1 } %ovf, 0
  %tmp24 = load i32, ptr %flag, align 4
  %ovf25 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %add, i32 %tmp24)
  %ovf.flag26 = extractvalue { i32, i1 } %ovf25, 1
  br i1 %ovf.flag26, label %panic27, label %arith.ok28

panic27:                                          ; preds = %arith.ok
  call void @kai_panic(ptr @kai.panic.msg.1, i64 16, ptr @kai.src.file, i64 20, i64 16)
  unreachable

arith.ok28:                                       ; preds = %arith.ok
  %add29 = extractvalue { i32, i1 } %ovf25, 0
  %call30 = call i32 @math.geometry.tag()
  %ovf31 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %add29, i32 %call30)
  %ovf.flag32 = extractvalue { i32, i1 } %ovf31, 1
  br i1 %ovf.flag32, label %panic33, label %arith.ok34

panic33:                                          ; preds = %arith.ok28
  call void @kai_panic(ptr @kai.panic.msg.2, i64 16, ptr @kai.src.file, i64 20, i64 16)
  unreachable

arith.ok34:                                       ; preds = %arith.ok28
  %add35 = extractvalue { i32, i1 } %ovf31, 0
  ret i32 %add35
}

define %math.geometry.Point @math.geometry.double(%math.geometry.Point %p) {
entry:
  %tmp = alloca %math.geometry.Point, align 8
  %p1 = alloca %math.geometry.Point, align 8
  store %math.geometry.Point %p, ptr %p1, align 4
  %field = getelementptr inbounds nuw %math.geometry.Point, ptr %p1, i32 0, i32 0
  %field2 = load i32, ptr %field, align 4
  %ovf = call { i32, i1 } @llvm.smul.with.overflow.i32(i32 %field2, i32 2)
  %ovf.flag = extractvalue { i32, i1 } %ovf, 1
  br i1 %ovf.flag, label %panic, label %arith.ok

panic:                                            ; preds = %entry
  call void @kai_panic(ptr @kai.panic.msg.3, i64 16, ptr @kai.src.file.4, i64 9, i64 23)
  unreachable

arith.ok:                                         ; preds = %entry
  %mul = extractvalue { i32, i1 } %ovf, 0
  %f = getelementptr inbounds nuw %math.geometry.Point, ptr %tmp, i32 0, i32 0
  store i32 %mul, ptr %f, align 4
  %field3 = getelementptr inbounds nuw %math.geometry.Point, ptr %p1, i32 0, i32 1
  %field4 = load i32, ptr %field3, align 4
  %ovf5 = call { i32, i1 } @llvm.smul.with.overflow.i32(i32 %field4, i32 2)
  %ovf.flag6 = extractvalue { i32, i1 } %ovf5, 1
  br i1 %ovf.flag6, label %panic7, label %arith.ok8

panic7:                                           ; preds = %arith.ok
  call void @kai_panic(ptr @kai.panic.msg.5, i64 16, ptr @kai.src.file.4, i64 9, i64 35)
  unreachable

arith.ok8:                                        ; preds = %arith.ok
  %mul9 = extractvalue { i32, i1 } %ovf5, 0
  %f10 = getelementptr inbounds nuw %math.geometry.Point, ptr %tmp, i32 0, i32 1
  store i32 %mul9, ptr %f10, align 4
  %lit = load %math.geometry.Point, ptr %tmp, align 4
  ret %math.geometry.Point %lit
}

define i32 @math.geometry.describe() {
entry:
  ret i32 100
}

define i32 @math.geometry.tag() {
entry:
  %call = call i32 @math.geometry.describe()
  br i1 false, label %panic, label %div.safe

panic:                                            ; preds = %entry
  call void @kai_panic(ptr @kai.panic.msg.6, i64 16, ptr @kai.src.file.4, i64 18, i64 12)
  unreachable

div.safe:                                         ; preds = %entry
  %lhs.min = icmp eq i32 %call, -2147483648
  %min.div = and i1 false, %lhs.min
  br i1 %min.div, label %panic1, label %safe.div

panic1:                                           ; preds = %div.safe
  call void @kai_panic(ptr @kai.panic.msg.7, i64 16, ptr @kai.src.file.4, i64 18, i64 12)
  unreachable

safe.div:                                         ; preds = %div.safe
  %div = sdiv i32 %call, 10
  ret i32 %div
}

define i32 @util.flags.describe() {
entry:
  ret i32 7
}

define i32 @util.flags.pick(i32 %n) {
entry:
  %n1 = alloca i32, align 4
  store i32 %n, ptr %n1, align 4
  %tmp = load i32, ptr %n1, align 4
  %gt = icmp sgt i32 %tmp, 0
  br i1 %gt, label %if.then, label %if.end

if.then:                                          ; preds = %entry
  %call = call i32 @util.flags.describe()
  ret i32 %call

if.end:                                           ; preds = %entry
  %call2 = call i32 @util.flags.describe()
  %ovf = call { i32, i1 } @llvm.ssub.with.overflow.i32(i32 0, i32 %call2)
  %ovf.flag = extractvalue { i32, i1 } %ovf, 1
  br i1 %ovf.flag, label %panic, label %arith.ok

panic:                                            ; preds = %if.end
  call void @kai_panic(ptr @kai.panic.msg.8, i64 16, ptr @kai.src.file.9, i64 14, i64 12)
  unreachable

arith.ok:                                         ; preds = %if.end
  %sub = extractvalue { i32, i1 } %ovf, 0
  ret i32 %sub
}

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.sadd.with.overflow.i32(i32, i32) #0

declare void @kai_panic(ptr, i64, ptr, i64, i64)

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.smul.with.overflow.i32(i32, i32) #0

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.ssub.with.overflow.i32(i32, i32) #0

attributes #0 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
