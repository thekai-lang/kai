; ModuleID = 'kai_module'
source_filename = "kai_module"

%"KaiArray.\22i32\22" = type { i64, i64, i64, ptr, ptr }
%Named = type { ptr, i32 }
%"KaiArray.\22ptr\22" = type { i64, i64, i64, ptr, ptr }

@kai.str = private unnamed_addr constant [3 x i8] c"kai"
@kai.str.1 = private unnamed_addr constant [3 x i8] c"kai"
@kai.str.2 = private unnamed_addr constant [3 x i8] c"kai"
@kai.str.3 = private unnamed_addr constant [3 x i8] c"KAI"
@kai.str.4 = private unnamed_addr constant [2 x i8] c"aa"
@kai.str.5 = private unnamed_addr constant [2 x i8] c"bb"
@kai.str.6 = private unnamed_addr constant [2 x i8] c"aa"
@kai.str.7 = private unnamed_addr constant [2 x i8] c"bb"
@kai.str.8 = private unnamed_addr constant [3 x i8] c"ada"
@kai.str.9 = private unnamed_addr constant [3 x i8] c"ada"
@kai.str.10 = private unnamed_addr constant [3 x i8] c"ada"

define ptr @id(ptr %s) {
entry:
  %s1 = alloca ptr, align 8
  store ptr %s, ptr %s1, align 8
  %tmp = load ptr, ptr %s1, align 8
  call void @kai_retain(ptr %tmp)
  ret ptr %tmp
}

define void @set_first(ptr %arr) {
entry:
  %arr1 = alloca ptr, align 8
  store ptr %arr, ptr %arr1, align 8
  %arr.hdr = load ptr, ptr %arr1, align 8
  %arr.elems.p = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %arr.hdr, i32 0, i32 3
  %arr.elems = load ptr, ptr %arr.elems.p, align 8
  %place.elem = getelementptr inbounds i32, ptr %arr.elems, i64 0
  store i32 42, ptr %place.elem, align 4
  ret void
}

define i32 @sum(ptr %a) {
entry:
  %for.idx = alloca i64, align 8
  %v = alloca i32, align 4
  %total = alloca i32, align 4
  %a1 = alloca ptr, align 8
  store ptr %a, ptr %a1, align 8
  store i32 0, ptr %total, align 4
  %tmp = load ptr, ptr %a1, align 8
  %arr.len.p = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %tmp, i32 0, i32 1
  %arr.len = load i64, ptr %arr.len.p, align 4
  store i64 0, ptr %for.idx, align 4
  br label %for.cond

for.cond:                                         ; preds = %for.body, %entry
  %for.i = load i64, ptr %for.idx, align 4
  %for.more = icmp slt i64 %for.i, %arr.len
  br i1 %for.more, label %for.body, label %for.end

for.body:                                         ; preds = %for.cond
  %arr.elems.p = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %tmp, i32 0, i32 3
  %arr.elems = load ptr, ptr %arr.elems.p, align 8
  %for.elem.slot = getelementptr inbounds i32, ptr %arr.elems, i64 %for.i
  %for.elem = load i32, ptr %for.elem.slot, align 4
  store i32 %for.elem, ptr %v, align 4
  %tmp2 = load i32, ptr %v, align 4
  %old = load i32, ptr %total, align 4
  %add = add i32 %old, %tmp2
  store i32 %add, ptr %total, align 4
  %for.next = add i64 %for.i, 1
  store i64 %for.next, ptr %for.idx, align 4
  br label %for.cond

for.end:                                          ; preds = %for.cond
  %tmp3 = load i32, ptr %total, align 4
  ret i32 %tmp3
}

define i32 @main() {
entry:
  %label = alloca ptr, align 8
  %n = alloca %Named, align 8
  %tmp65 = alloca %Named, align 8
  %nums = alloca ptr, align 8
  %words = alloca ptr, align 8
  %score = alloca i32, align 4
  %out = alloca ptr, align 8
  %lit = alloca ptr, align 8
  %str = call ptr @kai_string_new(ptr @kai.str, i64 3)
  store ptr %str, ptr %lit, align 8
  %tmp = load ptr, ptr %lit, align 8
  %call = call ptr @id(ptr %tmp)
  store ptr %call, ptr %out, align 8
  store i32 0, ptr %score, align 4
  %tmp1 = load ptr, ptr %lit, align 8
  %tmp2 = load ptr, ptr %out, align 8
  %str.eq = call i8 @kai_string_eq(ptr %tmp1, ptr %tmp2)
  %str.eq.b = trunc i8 %str.eq to i1
  br i1 %str.eq.b, label %and.rhs, label %and.end

and.rhs:                                          ; preds = %entry
  %tmp3 = load ptr, ptr %out, align 8
  %str4 = call ptr @kai_string_new(ptr @kai.str.1, i64 3)
  %str.eq5 = call i8 @kai_string_eq(ptr %tmp3, ptr %str4)
  %str.eq.b6 = trunc i8 %str.eq5 to i1
  br label %and.end

and.end:                                          ; preds = %and.rhs, %entry
  %and.result = phi i1 [ %str.eq.b, %entry ], [ %str.eq.b6, %and.rhs ]
  br i1 %and.result, label %and.rhs7, label %and.end8

and.rhs7:                                         ; preds = %and.end
  %str9 = call ptr @kai_string_new(ptr @kai.str.2, i64 3)
  %str10 = call ptr @kai_string_new(ptr @kai.str.3, i64 3)
  %str.eq11 = call i8 @kai_string_eq(ptr %str9, ptr %str10)
  %str.eq.b12 = trunc i8 %str.eq11 to i1
  %str.ne = xor i1 %str.eq.b12, true
  br label %and.end8

and.end8:                                         ; preds = %and.rhs7, %and.end
  %and.result13 = phi i1 [ %and.result, %and.end ], [ %str.ne, %and.rhs7 ]
  br i1 %and.result13, label %if.then, label %if.end

if.then:                                          ; preds = %and.end8
  %old = load i32, ptr %score, align 4
  %add = add i32 %old, 1
  store i32 %add, ptr %score, align 4
  br label %if.end

if.end:                                           ; preds = %if.then, %and.end8
  %arr = call ptr @kai_array_new(i64 2, i64 ptrtoint (ptr getelementptr (ptr, ptr null, i32 1) to i64), ptr @kai.dtor.elems_string)
  %arr.elems.p = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %arr, i32 0, i32 3
  %arr.elems = load ptr, ptr %arr.elems.p, align 8
  %str14 = call ptr @kai_string_new(ptr @kai.str.4, i64 2)
  %arr.slot = getelementptr inbounds ptr, ptr %arr.elems, i64 0
  store ptr %str14, ptr %arr.slot, align 8
  %str15 = call ptr @kai_string_new(ptr @kai.str.5, i64 2)
  %arr.slot16 = getelementptr inbounds ptr, ptr %arr.elems, i64 1
  store ptr %str15, ptr %arr.slot16, align 8
  store ptr %arr, ptr %words, align 8
  %arr.hdr = load ptr, ptr %words, align 8
  %arr.elems.p17 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %arr.hdr, i32 0, i32 3
  %arr.elems18 = load ptr, ptr %arr.elems.p17, align 8
  %place.elem = getelementptr inbounds ptr, ptr %arr.elems18, i64 0
  %tmp19 = load ptr, ptr %words, align 8
  %arr.elems.p20 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp19, i32 0, i32 3
  %arr.elems21 = load ptr, ptr %arr.elems.p20, align 8
  %elem.slot = getelementptr inbounds ptr, ptr %arr.elems21, i64 0
  %elem = load ptr, ptr %elem.slot, align 8
  call void @kai_retain(ptr %elem)
  %rel.hdr = load ptr, ptr %place.elem, align 8
  call void @kai_release(ptr %rel.hdr)
  store ptr %elem, ptr %place.elem, align 8
  %tmp22 = load ptr, ptr %words, align 8
  %arr.elems.p23 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp22, i32 0, i32 3
  %arr.elems24 = load ptr, ptr %arr.elems.p23, align 8
  %elem.slot25 = getelementptr inbounds ptr, ptr %arr.elems24, i64 0
  %elem26 = load ptr, ptr %elem.slot25, align 8
  %str27 = call ptr @kai_string_new(ptr @kai.str.6, i64 2)
  %str.eq28 = call i8 @kai_string_eq(ptr %elem26, ptr %str27)
  %str.eq.b29 = trunc i8 %str.eq28 to i1
  br i1 %str.eq.b29, label %and.rhs30, label %and.end31

and.rhs30:                                        ; preds = %if.end
  %tmp32 = load ptr, ptr %words, align 8
  %arr.elems.p33 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp32, i32 0, i32 3
  %arr.elems34 = load ptr, ptr %arr.elems.p33, align 8
  %elem.slot35 = getelementptr inbounds ptr, ptr %arr.elems34, i64 1
  %elem36 = load ptr, ptr %elem.slot35, align 8
  %str37 = call ptr @kai_string_new(ptr @kai.str.7, i64 2)
  %str.eq38 = call i8 @kai_string_eq(ptr %elem36, ptr %str37)
  %str.eq.b39 = trunc i8 %str.eq38 to i1
  br label %and.end31

and.end31:                                        ; preds = %and.rhs30, %if.end
  %and.result40 = phi i1 [ %str.eq.b29, %if.end ], [ %str.eq.b39, %and.rhs30 ]
  br i1 %and.result40, label %if.then41, label %if.end42

if.then41:                                        ; preds = %and.end31
  %old43 = load i32, ptr %score, align 4
  %add44 = add i32 %old43, 2
  store i32 %add44, ptr %score, align 4
  br label %if.end42

if.end42:                                         ; preds = %if.then41, %and.end31
  %arr45 = call ptr @kai_array_new(i64 3, i64 ptrtoint (ptr getelementptr (i32, ptr null, i32 1) to i64), ptr null)
  %arr.elems.p46 = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %arr45, i32 0, i32 3
  %arr.elems47 = load ptr, ptr %arr.elems.p46, align 8
  %arr.slot48 = getelementptr inbounds i32, ptr %arr.elems47, i64 0
  store i32 1, ptr %arr.slot48, align 4
  %arr.slot49 = getelementptr inbounds i32, ptr %arr.elems47, i64 1
  store i32 2, ptr %arr.slot49, align 4
  %arr.slot50 = getelementptr inbounds i32, ptr %arr.elems47, i64 2
  store i32 3, ptr %arr.slot50, align 4
  store ptr %arr45, ptr %nums, align 8
  %tmp51 = load ptr, ptr %nums, align 8
  call void @set_first(ptr %tmp51)
  %tmp52 = load ptr, ptr %nums, align 8
  %arr.elems.p53 = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %tmp52, i32 0, i32 3
  %arr.elems54 = load ptr, ptr %arr.elems.p53, align 8
  %elem.slot55 = getelementptr inbounds i32, ptr %arr.elems54, i64 0
  %elem56 = load i32, ptr %elem.slot55, align 4
  %eq = icmp eq i32 %elem56, 42
  br i1 %eq, label %if.then57, label %if.end58

if.then57:                                        ; preds = %if.end42
  %old59 = load i32, ptr %score, align 4
  %add60 = add i32 %old59, 4
  store i32 %add60, ptr %score, align 4
  br label %if.end58

if.end58:                                         ; preds = %if.then57, %if.end42
  %tmp61 = load ptr, ptr %nums, align 8
  %call62 = call i32 @sum(ptr %tmp61)
  %old63 = load i32, ptr %score, align 4
  %add64 = add i32 %old63, %call62
  store i32 %add64, ptr %score, align 4
  %str66 = call ptr @kai_string_new(ptr @kai.str.8, i64 3)
  %f = getelementptr inbounds nuw %Named, ptr %tmp65, i32 0, i32 0
  store ptr %str66, ptr %f, align 8
  %f67 = getelementptr inbounds nuw %Named, ptr %tmp65, i32 0, i32 1
  store i32 7, ptr %f67, align 4
  %lit68 = load %Named, ptr %tmp65, align 8
  store %Named %lit68, ptr %n, align 8
  %field = getelementptr inbounds nuw %Named, ptr %n, i32 0, i32 0
  %field69 = load ptr, ptr %field, align 8
  call void @kai_retain(ptr %field69)
  store ptr %field69, ptr %label, align 8
  %place = getelementptr inbounds nuw %Named, ptr %n, i32 0, i32 1
  store i32 8, ptr %place, align 4
  %tmp70 = load ptr, ptr %label, align 8
  %str71 = call ptr @kai_string_new(ptr @kai.str.9, i64 3)
  %str.eq72 = call i8 @kai_string_eq(ptr %tmp70, ptr %str71)
  %str.eq.b73 = trunc i8 %str.eq72 to i1
  br i1 %str.eq.b73, label %and.rhs74, label %and.end75

and.rhs74:                                        ; preds = %if.end58
  %field76 = getelementptr inbounds nuw %Named, ptr %n, i32 0, i32 0
  %field77 = load ptr, ptr %field76, align 8
  %str78 = call ptr @kai_string_new(ptr @kai.str.10, i64 3)
  %str.eq79 = call i8 @kai_string_eq(ptr %field77, ptr %str78)
  %str.eq.b80 = trunc i8 %str.eq79 to i1
  br label %and.end75

and.end75:                                        ; preds = %and.rhs74, %if.end58
  %and.result81 = phi i1 [ %str.eq.b73, %if.end58 ], [ %str.eq.b80, %and.rhs74 ]
  br i1 %and.result81, label %and.rhs82, label %and.end83

and.rhs82:                                        ; preds = %and.end75
  %field84 = getelementptr inbounds nuw %Named, ptr %n, i32 0, i32 1
  %field85 = load i32, ptr %field84, align 4
  %eq86 = icmp eq i32 %field85, 8
  br label %and.end83

and.end83:                                        ; preds = %and.rhs82, %and.end75
  %and.result87 = phi i1 [ %and.result81, %and.end75 ], [ %eq86, %and.rhs82 ]
  br i1 %and.result87, label %if.then88, label %if.end89

if.then88:                                        ; preds = %and.end83
  %old90 = load i32, ptr %score, align 4
  %add91 = add i32 %old90, 10
  store i32 %add91, ptr %score, align 4
  br label %if.end89

if.end89:                                         ; preds = %if.then88, %and.end83
  %tmp92 = load i32, ptr %score, align 4
  %rel.hdr93 = load ptr, ptr %label, align 8
  call void @kai_release(ptr %rel.hdr93)
  call void @kai.release_Named(ptr %n)
  %rel.hdr94 = load ptr, ptr %nums, align 8
  call void @kai_release(ptr %rel.hdr94)
  %rel.hdr95 = load ptr, ptr %words, align 8
  call void @kai_release(ptr %rel.hdr95)
  %rel.hdr96 = load ptr, ptr %out, align 8
  call void @kai_release(ptr %rel.hdr96)
  %rel.hdr97 = load ptr, ptr %lit, align 8
  call void @kai_release(ptr %rel.hdr97)
  ret i32 %tmp92
}

declare void @kai_retain(ptr)

declare ptr @kai_string_new(ptr, i64)

declare i8 @kai_string_eq(ptr, ptr)

declare ptr @kai_array_new(i64, i64, ptr)

define void @kai.dtor.elems_string(ptr %0) {
entry:
  %dtor.len.p = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %0, i32 0, i32 1
  %dtor.len = load i64, ptr %dtor.len.p, align 4
  %arr.elems.p = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %0, i32 0, i32 3
  %arr.elems = load ptr, ptr %arr.elems.p, align 8
  %dtor.i = alloca i64, align 8
  store i64 0, ptr %dtor.i, align 4
  br label %dtor.loop

dtor.loop:                                        ; preds = %dtor.body, %entry
  %dtor.i.v = load i64, ptr %dtor.i, align 4
  %dtor.more = icmp slt i64 %dtor.i.v, %dtor.len
  br i1 %dtor.more, label %dtor.body, label %dtor.done

dtor.body:                                        ; preds = %dtor.loop
  %dtor.elem = getelementptr inbounds ptr, ptr %arr.elems, i64 %dtor.i.v
  %rel.hdr = load ptr, ptr %dtor.elem, align 8
  call void @kai_release(ptr %rel.hdr)
  %dtor.next = add i64 %dtor.i.v, 1
  store i64 %dtor.next, ptr %dtor.i, align 4
  br label %dtor.loop

dtor.done:                                        ; preds = %dtor.loop
  ret void
}

declare void @kai_release(ptr)

define void @kai.release_Named(ptr %0) {
entry:
  %fld = getelementptr inbounds nuw %Named, ptr %0, i32 0, i32 0
  %rel.hdr = load ptr, ptr %fld, align 8
  call void @kai_release(ptr %rel.hdr)
  ret void
}
