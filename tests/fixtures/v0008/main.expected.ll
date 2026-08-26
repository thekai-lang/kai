; ModuleID = 'kai_module'
source_filename = "kai_module"

@kai.sink.path = private unnamed_addr constant [35 x i8] c"/kai-fixture-root/.kai/observe.log\00", align 1
@kai.sig.loc = private unnamed_addr constant [14 x i8] c"main.kai:7:13\00", align 1
@kai.sig.cond = private unnamed_addr constant [8 x i8] c"age > 0\00", align 1
@kai.sink.path.1 = private unnamed_addr constant [32 x i8] c"/kai-fixture-root/.kai/debt.log\00", align 1
@kai.sig.loc.2 = private unnamed_addr constant [15 x i8] c"main.kai:15:17\00", align 1
@kai.sig.cond.3 = private unnamed_addr constant [6 x i8] c"a > 0\00", align 1
@kai.require.msg = private unnamed_addr constant [28 x i8] c"requirement violated: a > 0\00", align 1
@kai.src.file = private unnamed_addr constant [9 x i8] c"main.kai\00", align 1
@kai.panic.msg = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.sink.path.4 = private unnamed_addr constant [35 x i8] c"/kai-fixture-root/.kai/observe.log\00", align 1
@kai.sig.loc.5 = private unnamed_addr constant [15 x i8] c"main.kai:19:13\00", align 1
@kai.sig.cond.6 = private unnamed_addr constant [12 x i8] c"total == 10\00", align 1

define void @check(i32 %age) {
entry:
  %age1 = alloca i32, align 4
  store i32 %age, ptr %age1, align 4
  %tmp = load i32, ptr %age1, align 4
  %gt = icmp sgt i32 %tmp, 0
  %observe.outcome.i32 = sext i1 %gt to i32
  call void @kai_observe_record(ptr @kai.sink.path, ptr @kai.sig.loc, ptr @kai.sig.cond, i32 %observe.outcome.i32)
  ret void
}

define i32 @main() {
entry:
  %a = alloca i32, align 4
  %total = alloca i32, align 4
  store i32 0, ptr %total, align 4
  store i32 5, ptr %a, align 4
  %tmp = load i32, ptr %a, align 4
  %gt = icmp sgt i32 %tmp, 0
  br i1 %gt, label %if.then, label %if.end

if.then:                                          ; preds = %entry
  %tmp1 = load i32, ptr %a, align 4
  %gt2 = icmp sgt i32 %tmp1, 0
  br i1 %gt2, label %require.ok, label %require.viol

if.end:                                           ; preds = %arith.ok, %entry
  %tmp3 = load i32, ptr %a, align 4
  call void @check(i32 %tmp3)
  %tmp4 = load i32, ptr %total, align 4
  %eq = icmp eq i32 %tmp4, 10
  %observe.outcome.i32 = sext i1 %eq to i32
  call void @kai_observe_record(ptr @kai.sink.path.4, ptr @kai.sig.loc.5, ptr @kai.sig.cond.6, i32 %observe.outcome.i32)
  %tmp5 = load i32, ptr %total, align 4
  ret i32 %tmp5

require.ok:                                       ; preds = %if.then
  %old = load i32, ptr %total, align 4
  %ovf = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old, i32 10)
  %ovf.flag = extractvalue { i32, i1 } %ovf, 1
  br i1 %ovf.flag, label %panic, label %arith.ok

require.viol:                                     ; preds = %if.then
  call void @kai_debt_record(ptr @kai.sink.path.1, ptr @kai.sig.loc.2, ptr @kai.sig.cond.3)
  call void @kai_panic(ptr @kai.require.msg, i64 27, ptr @kai.src.file, i64 15, i64 17)
  unreachable

panic:                                            ; preds = %require.ok
  call void @kai_panic(ptr @kai.panic.msg, i64 16, ptr @kai.src.file, i64 16, i64 9)
  unreachable

arith.ok:                                         ; preds = %require.ok
  %add = extractvalue { i32, i1 } %ovf, 0
  store i32 %add, ptr %total, align 4
  br label %if.end
}

declare void @kai_observe_record(ptr, ptr, ptr, i32)

declare void @kai_debt_record(ptr, ptr, ptr)

declare void @kai_panic(ptr, i64, ptr, i64, i64)

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.sadd.with.overflow.i32(i32, i32) #0

attributes #0 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
