; ModuleID = 'kai_module'
source_filename = "kai_module"

%Point = type { i32, i32 }
%Segment = type { %Point, %Point }

define i32 @shift(%Point %p, i32 %dx) {
entry:
  %dx2 = alloca i32, align 4
  %p1 = alloca %Point, align 8
  store %Point %p, ptr %p1, align 4
  store i32 %dx, ptr %dx2, align 4
  %place = getelementptr inbounds nuw %Point, ptr %p1, i32 0, i32 0
  %tmp = load i32, ptr %dx2, align 4
  %old = load i32, ptr %place, align 4
  %add = add i32 %old, %tmp
  store i32 %add, ptr %place, align 4
  %field = getelementptr inbounds nuw %Point, ptr %p1, i32 0, i32 0
  %field3 = load i32, ptr %field, align 4
  %field4 = getelementptr inbounds nuw %Point, ptr %p1, i32 0, i32 1
  %field5 = load i32, ptr %field4, align 4
  %add6 = add i32 %field3, %field5
  ret i32 %add6
}

define i32 @main() {
entry:
  %moved = alloca i32, align 4
  %seg = alloca %Segment, align 8
  %tmp4 = alloca %Point, align 8
  %tmp1 = alloca %Point, align 8
  %tmp = alloca %Segment, align 8
  %f = getelementptr inbounds nuw %Point, ptr %tmp1, i32 0, i32 0
  store i32 1, ptr %f, align 4
  %f2 = getelementptr inbounds nuw %Point, ptr %tmp1, i32 0, i32 1
  store i32 2, ptr %f2, align 4
  %lit = load %Point, ptr %tmp1, align 4
  %f3 = getelementptr inbounds nuw %Segment, ptr %tmp, i32 0, i32 0
  store %Point %lit, ptr %f3, align 4
  %f5 = getelementptr inbounds nuw %Point, ptr %tmp4, i32 0, i32 0
  store i32 30, ptr %f5, align 4
  %f6 = getelementptr inbounds nuw %Point, ptr %tmp4, i32 0, i32 1
  store i32 4, ptr %f6, align 4
  %lit7 = load %Point, ptr %tmp4, align 4
  %f8 = getelementptr inbounds nuw %Segment, ptr %tmp, i32 0, i32 1
  store %Point %lit7, ptr %f8, align 4
  %lit9 = load %Segment, ptr %tmp, align 4
  store %Segment %lit9, ptr %seg, align 4
  %field = getelementptr inbounds nuw %Segment, ptr %seg, i32 0, i32 1
  %field10 = load %Point, ptr %field, align 4
  %call = call i32 @shift(%Point %field10, i32 5)
  store i32 %call, ptr %moved, align 4
  %place = getelementptr inbounds nuw %Segment, ptr %seg, i32 0, i32 0
  %place11 = getelementptr inbounds nuw %Point, ptr %place, i32 0, i32 1
  store i32 20, ptr %place11, align 4
  %tmp12 = load i32, ptr %moved, align 4
  %eq = icmp eq i32 %tmp12, 39
  br i1 %eq, label %and.rhs, label %and.end

and.rhs:                                          ; preds = %entry
  %place13 = getelementptr inbounds nuw %Segment, ptr %seg, i32 0, i32 0
  %field14 = getelementptr inbounds nuw %Point, ptr %place13, i32 0, i32 0
  %field15 = load i32, ptr %field14, align 4
  %eq16 = icmp eq i32 %field15, 1
  br label %and.end

and.end:                                          ; preds = %and.rhs, %entry
  %and.result = phi i1 [ %eq, %entry ], [ %eq16, %and.rhs ]
  br i1 %and.result, label %and.rhs17, label %and.end18

and.rhs17:                                        ; preds = %and.end
  %place19 = getelementptr inbounds nuw %Segment, ptr %seg, i32 0, i32 0
  %field20 = getelementptr inbounds nuw %Point, ptr %place19, i32 0, i32 1
  %field21 = load i32, ptr %field20, align 4
  %eq22 = icmp eq i32 %field21, 20
  br label %and.end18

and.end18:                                        ; preds = %and.rhs17, %and.end
  %and.result23 = phi i1 [ %and.result, %and.end ], [ %eq22, %and.rhs17 ]
  br i1 %and.result23, label %and.rhs24, label %and.end25

and.rhs24:                                        ; preds = %and.end18
  %place26 = getelementptr inbounds nuw %Segment, ptr %seg, i32 0, i32 1
  %field27 = getelementptr inbounds nuw %Point, ptr %place26, i32 0, i32 0
  %field28 = load i32, ptr %field27, align 4
  %eq29 = icmp eq i32 %field28, 30
  br label %and.end25

and.end25:                                        ; preds = %and.rhs24, %and.end18
  %and.result30 = phi i1 [ %and.result23, %and.end18 ], [ %eq29, %and.rhs24 ]
  br i1 %and.result30, label %if.then, label %if.end

if.then:                                          ; preds = %and.end25
  %tmp31 = load i32, ptr %moved, align 4
  ret i32 %tmp31

if.end:                                           ; preds = %and.end25
  ret i32 0
}
