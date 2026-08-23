; ModuleID = 'kai_module'
source_filename = "kai_module"

%math.geometry.Point = type { i32, i32 }

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
  %add = add i32 %field21, %field23
  %tmp24 = load i32, ptr %flag, align 4
  %add25 = add i32 %add, %tmp24
  %call26 = call i32 @math.geometry.tag()
  %add27 = add i32 %add25, %call26
  ret i32 %add27

if.end:                                           ; preds = %and.end16
  ret i32 0
}

define %math.geometry.Point @math.geometry.double(%math.geometry.Point %p) {
entry:
  %tmp = alloca %math.geometry.Point, align 8
  %p1 = alloca %math.geometry.Point, align 8
  store %math.geometry.Point %p, ptr %p1, align 4
  %field = getelementptr inbounds nuw %math.geometry.Point, ptr %p1, i32 0, i32 0
  %field2 = load i32, ptr %field, align 4
  %mul = mul i32 %field2, 2
  %f = getelementptr inbounds nuw %math.geometry.Point, ptr %tmp, i32 0, i32 0
  store i32 %mul, ptr %f, align 4
  %field3 = getelementptr inbounds nuw %math.geometry.Point, ptr %p1, i32 0, i32 1
  %field4 = load i32, ptr %field3, align 4
  %mul5 = mul i32 %field4, 2
  %f6 = getelementptr inbounds nuw %math.geometry.Point, ptr %tmp, i32 0, i32 1
  store i32 %mul5, ptr %f6, align 4
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
  %sub = sub i32 0, %call2
  ret i32 %sub
}
