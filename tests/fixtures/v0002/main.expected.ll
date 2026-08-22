; ModuleID = 'kai_module'
source_filename = "kai_module"

define i32 @main() {
entry:
  %total = alloca i32, align 4
  %base = alloca i32, align 4
  store i32 6, ptr %base, align 4
  %tmp = load i32, ptr %base, align 4
  %mul = mul i32 %tmp, 7
  store i32 %mul, ptr %total, align 4
  %old = load i32, ptr %total, align 4
  %add = add i32 %old, 1
  store i32 %add, ptr %total, align 4
  %tmp1 = load i32, ptr %total, align 4
  %ge = icmp sge i32 %tmp1, 42
  br i1 %ge, label %and.rhs, label %and.end

and.rhs:                                          ; preds = %entry
  %tmp2 = load i32, ptr %total, align 4
  %gt = icmp sgt i32 %tmp2, 43
  %not = xor i1 %gt, true
  br label %and.end

and.end:                                          ; preds = %and.rhs, %entry
  %and.result = phi i1 [ %ge, %entry ], [ %not, %and.rhs ]
  br i1 %and.result, label %if.then, label %if.else

if.then:                                          ; preds = %and.end
  %tmp3 = load i32, ptr %total, align 4
  %sub = sub i32 %tmp3, 1
  ret i32 %sub

if.else:                                          ; preds = %and.end
  ret i32 0

if.end:                                           ; No predecessors!
  ret i32 0
}
