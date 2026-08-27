; ModuleID = 'kai_module'
source_filename = "kai_module"

%"KaiArray.\22i32\22" = type { i64, i64, i64, ptr, ptr }
%Named = type { ptr, i32 }
%"KaiArray.\22ptr\22" = type { i64, i64, i64, ptr, ptr }

@kai.panic.msg = private unnamed_addr constant [26 x i8] c"array index out of bounds\00", align 1
@kai.src.file = private unnamed_addr constant [9 x i8] c"main.kai\00", align 1
@kai.panic.msg.1 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.str = private unnamed_addr constant [3 x i8] c"kai"
@kai.str.2 = private unnamed_addr constant [3 x i8] c"kai"
@kai.str.3 = private unnamed_addr constant [3 x i8] c"kai"
@kai.str.4 = private unnamed_addr constant [3 x i8] c"KAI"
@kai.panic.msg.5 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.str.6 = private unnamed_addr constant [2 x i8] c"aa"
@kai.str.7 = private unnamed_addr constant [2 x i8] c"bb"
@kai.panic.msg.8 = private unnamed_addr constant [26 x i8] c"array index out of bounds\00", align 1
@kai.panic.msg.9 = private unnamed_addr constant [26 x i8] c"array index out of bounds\00", align 1
@kai.str.10 = private unnamed_addr constant [2 x i8] c"aa"
@kai.panic.msg.11 = private unnamed_addr constant [26 x i8] c"array index out of bounds\00", align 1
@kai.str.12 = private unnamed_addr constant [2 x i8] c"bb"
@kai.panic.msg.13 = private unnamed_addr constant [26 x i8] c"array index out of bounds\00", align 1
@kai.panic.msg.14 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.panic.msg.15 = private unnamed_addr constant [26 x i8] c"array index out of bounds\00", align 1
@kai.panic.msg.16 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.panic.msg.17 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.str.18 = private unnamed_addr constant [3 x i8] c"ada"
@kai.str.19 = private unnamed_addr constant [3 x i8] c"ada"
@kai.str.20 = private unnamed_addr constant [3 x i8] c"ada"
@kai.panic.msg.21 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1

define ptr @id(ptr %s) {
entry:
  %s1 = alloca ptr, align 8
  store ptr %s, ptr %s1, align 8
  %ret.hdr = load ptr, ptr %s1, align 8
  call void @kai_retain(ptr %ret.hdr)
  %tmp = load ptr, ptr %s1, align 8
  call void @kai_retain(ptr %tmp)
  %rel.hdr = load ptr, ptr %s1, align 8
  call void @kai_release(ptr %rel.hdr)
  ret ptr %tmp
}

define void @set_first(ptr %arr) {
entry:
  %arr1 = alloca ptr, align 8
  store ptr %arr, ptr %arr1, align 8
  %ret.hdr = load ptr, ptr %arr1, align 8
  call void @kai_retain(ptr %ret.hdr)
  %arr.hdr = load ptr, ptr %arr1, align 8
  %arr.len.p = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %arr.hdr, i32 0, i32 1
  %arr.len = load i64, ptr %arr.len.p, align 4
  %bnd.high = icmp slt i64 0, %arr.len
  %bnd.ok = and i1 true, %bnd.high
  %bnd.bad = xor i1 %bnd.ok, true
  br i1 %bnd.bad, label %panic, label %in.bounds

panic:                                            ; preds = %entry
  call void @kai_panic(ptr @kai.panic.msg, i64 25, ptr @kai.src.file, i64 23, i64 9)
  unreachable

in.bounds:                                        ; preds = %entry
  %arr.elems.p = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %arr.hdr, i32 0, i32 3
  %arr.elems = load ptr, ptr %arr.elems.p, align 8
  %place.elem = getelementptr inbounds i32, ptr %arr.elems, i64 0
  store i32 42, ptr %place.elem, align 4
  %rel.hdr = load ptr, ptr %arr1, align 8
  call void @kai_release(ptr %rel.hdr)
  ret void
}

define i32 @sum(ptr %a) {
entry:
  %for.idx = alloca i64, align 8
  %v = alloca i32, align 4
  %total = alloca i32, align 4
  %a1 = alloca ptr, align 8
  store ptr %a, ptr %a1, align 8
  %ret.hdr = load ptr, ptr %a1, align 8
  call void @kai_retain(ptr %ret.hdr)
  store i32 0, ptr %total, align 4
  %tmp = load ptr, ptr %a1, align 8
  %arr.len.p = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %tmp, i32 0, i32 1
  %arr.len = load i64, ptr %arr.len.p, align 4
  store i64 0, ptr %for.idx, align 4
  br label %for.cond

for.cond:                                         ; preds = %arith.ok, %entry
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
  %ovf = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old, i32 %tmp2)
  %ovf.flag = extractvalue { i32, i1 } %ovf, 1
  br i1 %ovf.flag, label %panic, label %arith.ok

for.end:                                          ; preds = %for.cond
  %tmp3 = load i32, ptr %total, align 4
  %rel.hdr = load ptr, ptr %a1, align 8
  call void @kai_release(ptr %rel.hdr)
  ret i32 %tmp3

panic:                                            ; preds = %for.body
  call void @kai_panic(ptr @kai.panic.msg.1, i64 16, ptr @kai.src.file, i64 29, i64 18)
  unreachable

arith.ok:                                         ; preds = %for.body
  %add = extractvalue { i32, i1 } %ovf, 0
  store i32 %add, ptr %total, align 4
  %for.next = add i64 %for.i, 1
  store i64 %for.next, ptr %for.idx, align 4
  br label %for.cond
}

define i32 @main() {
entry:
  %"$tmp133" = alloca ptr, align 8
  %"$tmp125" = alloca ptr, align 8
  %label = alloca ptr, align 8
  %n = alloca %Named, align 8
  %tmp119 = alloca %Named, align 8
  %nums = alloca ptr, align 8
  %"$tmp58" = alloca ptr, align 8
  %"$tmp39" = alloca ptr, align 8
  %words = alloca ptr, align 8
  %"$tmp13" = alloca ptr, align 8
  %"$tmp11" = alloca ptr, align 8
  %"$tmp" = alloca ptr, align 8
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
  %str3 = call ptr @kai_string_new(ptr @kai.str.2, i64 3)
  store ptr %str3, ptr %"$tmp", align 8
  %tmp4 = load ptr, ptr %out, align 8
  %tmp5 = load ptr, ptr %"$tmp", align 8
  %str.eq6 = call i8 @kai_string_eq(ptr %tmp4, ptr %tmp5)
  %str.eq.b7 = trunc i8 %str.eq6 to i1
  %rel.hdr = load ptr, ptr %"$tmp", align 8
  call void @kai_release(ptr %rel.hdr)
  br label %and.end

and.end:                                          ; preds = %and.rhs, %entry
  %and.result = phi i1 [ %str.eq.b, %entry ], [ %str.eq.b7, %and.rhs ]
  br i1 %and.result, label %and.rhs8, label %and.end9

and.rhs8:                                         ; preds = %and.end
  %str10 = call ptr @kai_string_new(ptr @kai.str.3, i64 3)
  store ptr %str10, ptr %"$tmp11", align 8
  %str12 = call ptr @kai_string_new(ptr @kai.str.4, i64 3)
  store ptr %str12, ptr %"$tmp13", align 8
  %tmp14 = load ptr, ptr %"$tmp11", align 8
  %tmp15 = load ptr, ptr %"$tmp13", align 8
  %str.eq16 = call i8 @kai_string_eq(ptr %tmp14, ptr %tmp15)
  %str.eq.b17 = trunc i8 %str.eq16 to i1
  %str.ne = xor i1 %str.eq.b17, true
  %rel.hdr18 = load ptr, ptr %"$tmp11", align 8
  call void @kai_release(ptr %rel.hdr18)
  %rel.hdr19 = load ptr, ptr %"$tmp13", align 8
  call void @kai_release(ptr %rel.hdr19)
  br label %and.end9

and.end9:                                         ; preds = %and.rhs8, %and.end
  %and.result20 = phi i1 [ %and.result, %and.end ], [ %str.ne, %and.rhs8 ]
  br i1 %and.result20, label %if.then, label %if.end

if.then:                                          ; preds = %and.end9
  %old = load i32, ptr %score, align 4
  %ovf = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old, i32 1)
  %ovf.flag = extractvalue { i32, i1 } %ovf, 1
  br i1 %ovf.flag, label %panic, label %arith.ok

if.end:                                           ; preds = %arith.ok, %and.end9
  %arr = call ptr @kai_array_new(i64 2, i64 ptrtoint (ptr getelementptr (ptr, ptr null, i32 1) to i64), ptr @kai.dtor.elems_string)
  %arr.elems.p = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %arr, i32 0, i32 3
  %arr.elems = load ptr, ptr %arr.elems.p, align 8
  %str21 = call ptr @kai_string_new(ptr @kai.str.6, i64 2)
  %arr.slot = getelementptr inbounds ptr, ptr %arr.elems, i64 0
  store ptr %str21, ptr %arr.slot, align 8
  %str22 = call ptr @kai_string_new(ptr @kai.str.7, i64 2)
  %arr.slot23 = getelementptr inbounds ptr, ptr %arr.elems, i64 1
  store ptr %str22, ptr %arr.slot23, align 8
  store ptr %arr, ptr %words, align 8
  %arr.hdr = load ptr, ptr %words, align 8
  %arr.len.p = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %arr.hdr, i32 0, i32 1
  %arr.len = load i64, ptr %arr.len.p, align 4
  %bnd.high = icmp slt i64 0, %arr.len
  %bnd.ok = and i1 true, %bnd.high
  %bnd.bad = xor i1 %bnd.ok, true
  br i1 %bnd.bad, label %panic24, label %in.bounds

panic:                                            ; preds = %if.then
  call void @kai_panic(ptr @kai.panic.msg.5, i64 16, ptr @kai.src.file, i64 39, i64 9)
  unreachable

arith.ok:                                         ; preds = %if.then
  %add = extractvalue { i32, i1 } %ovf, 0
  store i32 %add, ptr %score, align 4
  br label %if.end

panic24:                                          ; preds = %if.end
  call void @kai_panic(ptr @kai.panic.msg.8, i64 25, ptr @kai.src.file, i64 44, i64 11)
  unreachable

in.bounds:                                        ; preds = %if.end
  %arr.elems.p25 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %arr.hdr, i32 0, i32 3
  %arr.elems26 = load ptr, ptr %arr.elems.p25, align 8
  %place.elem = getelementptr inbounds ptr, ptr %arr.elems26, i64 0
  %tmp27 = load ptr, ptr %words, align 8
  %arr.len.p28 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp27, i32 0, i32 1
  %arr.len29 = load i64, ptr %arr.len.p28, align 4
  %bnd.high30 = icmp slt i64 0, %arr.len29
  %bnd.ok31 = and i1 true, %bnd.high30
  %bnd.bad32 = xor i1 %bnd.ok31, true
  br i1 %bnd.bad32, label %panic33, label %in.bounds34

panic33:                                          ; preds = %in.bounds
  call void @kai_panic(ptr @kai.panic.msg.9, i64 25, ptr @kai.src.file, i64 44, i64 16)
  unreachable

in.bounds34:                                      ; preds = %in.bounds
  %arr.elems.p35 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp27, i32 0, i32 3
  %arr.elems36 = load ptr, ptr %arr.elems.p35, align 8
  %elem.slot = getelementptr inbounds ptr, ptr %arr.elems36, i64 0
  %elem = load ptr, ptr %elem.slot, align 8
  call void @kai_retain(ptr %elem)
  %rel.hdr37 = load ptr, ptr %place.elem, align 8
  call void @kai_release(ptr %rel.hdr37)
  store ptr %elem, ptr %place.elem, align 8
  %str38 = call ptr @kai_string_new(ptr @kai.str.10, i64 2)
  store ptr %str38, ptr %"$tmp39", align 8
  %tmp40 = load ptr, ptr %words, align 8
  %arr.len.p41 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp40, i32 0, i32 1
  %arr.len42 = load i64, ptr %arr.len.p41, align 4
  %bnd.high43 = icmp slt i64 0, %arr.len42
  %bnd.ok44 = and i1 true, %bnd.high43
  %bnd.bad45 = xor i1 %bnd.ok44, true
  br i1 %bnd.bad45, label %panic46, label %in.bounds47

panic46:                                          ; preds = %in.bounds34
  call void @kai_panic(ptr @kai.panic.msg.11, i64 25, ptr @kai.src.file, i64 45, i64 8)
  unreachable

in.bounds47:                                      ; preds = %in.bounds34
  %arr.elems.p48 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp40, i32 0, i32 3
  %arr.elems49 = load ptr, ptr %arr.elems.p48, align 8
  %elem.slot50 = getelementptr inbounds ptr, ptr %arr.elems49, i64 0
  %elem51 = load ptr, ptr %elem.slot50, align 8
  %tmp52 = load ptr, ptr %"$tmp39", align 8
  %str.eq53 = call i8 @kai_string_eq(ptr %elem51, ptr %tmp52)
  %str.eq.b54 = trunc i8 %str.eq53 to i1
  br i1 %str.eq.b54, label %and.rhs55, label %and.end56

and.rhs55:                                        ; preds = %in.bounds47
  %str57 = call ptr @kai_string_new(ptr @kai.str.12, i64 2)
  store ptr %str57, ptr %"$tmp58", align 8
  %tmp59 = load ptr, ptr %words, align 8
  %arr.len.p60 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp59, i32 0, i32 1
  %arr.len61 = load i64, ptr %arr.len.p60, align 4
  %bnd.high62 = icmp slt i64 1, %arr.len61
  %bnd.ok63 = and i1 true, %bnd.high62
  %bnd.bad64 = xor i1 %bnd.ok63, true
  br i1 %bnd.bad64, label %panic65, label %in.bounds66

and.end56:                                        ; preds = %in.bounds66, %in.bounds47
  %and.result75 = phi i1 [ %str.eq.b54, %in.bounds47 ], [ %str.eq.b73, %in.bounds66 ]
  br i1 %and.result75, label %if.then76, label %if.end77

panic65:                                          ; preds = %and.rhs55
  call void @kai_panic(ptr @kai.panic.msg.13, i64 25, ptr @kai.src.file, i64 45, i64 28)
  unreachable

in.bounds66:                                      ; preds = %and.rhs55
  %arr.elems.p67 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp59, i32 0, i32 3
  %arr.elems68 = load ptr, ptr %arr.elems.p67, align 8
  %elem.slot69 = getelementptr inbounds ptr, ptr %arr.elems68, i64 1
  %elem70 = load ptr, ptr %elem.slot69, align 8
  %tmp71 = load ptr, ptr %"$tmp58", align 8
  %str.eq72 = call i8 @kai_string_eq(ptr %elem70, ptr %tmp71)
  %str.eq.b73 = trunc i8 %str.eq72 to i1
  %rel.hdr74 = load ptr, ptr %"$tmp58", align 8
  call void @kai_release(ptr %rel.hdr74)
  br label %and.end56

if.then76:                                        ; preds = %and.end56
  %old78 = load i32, ptr %score, align 4
  %ovf79 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old78, i32 2)
  %ovf.flag80 = extractvalue { i32, i1 } %ovf79, 1
  br i1 %ovf.flag80, label %panic81, label %arith.ok82

if.end77:                                         ; preds = %arith.ok82, %and.end56
  %arr84 = call ptr @kai_array_new(i64 3, i64 ptrtoint (ptr getelementptr (i32, ptr null, i32 1) to i64), ptr null)
  %arr.elems.p85 = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %arr84, i32 0, i32 3
  %arr.elems86 = load ptr, ptr %arr.elems.p85, align 8
  %arr.slot87 = getelementptr inbounds i32, ptr %arr.elems86, i64 0
  store i32 1, ptr %arr.slot87, align 4
  %arr.slot88 = getelementptr inbounds i32, ptr %arr.elems86, i64 1
  store i32 2, ptr %arr.slot88, align 4
  %arr.slot89 = getelementptr inbounds i32, ptr %arr.elems86, i64 2
  store i32 3, ptr %arr.slot89, align 4
  store ptr %arr84, ptr %nums, align 8
  %tmp90 = load ptr, ptr %nums, align 8
  call void @set_first(ptr %tmp90)
  %tmp91 = load ptr, ptr %nums, align 8
  %arr.len.p92 = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %tmp91, i32 0, i32 1
  %arr.len93 = load i64, ptr %arr.len.p92, align 4
  %bnd.high94 = icmp slt i64 0, %arr.len93
  %bnd.ok95 = and i1 true, %bnd.high94
  %bnd.bad96 = xor i1 %bnd.ok95, true
  br i1 %bnd.bad96, label %panic97, label %in.bounds98

panic81:                                          ; preds = %if.then76
  call void @kai_panic(ptr @kai.panic.msg.14, i64 16, ptr @kai.src.file, i64 46, i64 9)
  unreachable

arith.ok82:                                       ; preds = %if.then76
  %add83 = extractvalue { i32, i1 } %ovf79, 0
  store i32 %add83, ptr %score, align 4
  br label %if.end77

panic97:                                          ; preds = %if.end77
  call void @kai_panic(ptr @kai.panic.msg.15, i64 25, ptr @kai.src.file, i64 53, i64 8)
  unreachable

in.bounds98:                                      ; preds = %if.end77
  %arr.elems.p99 = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %tmp91, i32 0, i32 3
  %arr.elems100 = load ptr, ptr %arr.elems.p99, align 8
  %elem.slot101 = getelementptr inbounds i32, ptr %arr.elems100, i64 0
  %elem102 = load i32, ptr %elem.slot101, align 4
  %eq = icmp eq i32 %elem102, 42
  br i1 %eq, label %if.then103, label %if.end104

if.then103:                                       ; preds = %in.bounds98
  %old105 = load i32, ptr %score, align 4
  %ovf106 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old105, i32 4)
  %ovf.flag107 = extractvalue { i32, i1 } %ovf106, 1
  br i1 %ovf.flag107, label %panic108, label %arith.ok109

if.end104:                                        ; preds = %arith.ok109, %in.bounds98
  %tmp111 = load ptr, ptr %nums, align 8
  %call112 = call i32 @sum(ptr %tmp111)
  %old113 = load i32, ptr %score, align 4
  %ovf114 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old113, i32 %call112)
  %ovf.flag115 = extractvalue { i32, i1 } %ovf114, 1
  br i1 %ovf.flag115, label %panic116, label %arith.ok117

panic108:                                         ; preds = %if.then103
  call void @kai_panic(ptr @kai.panic.msg.16, i64 16, ptr @kai.src.file, i64 54, i64 9)
  unreachable

arith.ok109:                                      ; preds = %if.then103
  %add110 = extractvalue { i32, i1 } %ovf106, 0
  store i32 %add110, ptr %score, align 4
  br label %if.end104

panic116:                                         ; preds = %if.end104
  call void @kai_panic(ptr @kai.panic.msg.17, i64 16, ptr @kai.src.file, i64 59, i64 5)
  unreachable

arith.ok117:                                      ; preds = %if.end104
  %add118 = extractvalue { i32, i1 } %ovf114, 0
  store i32 %add118, ptr %score, align 4
  %str120 = call ptr @kai_string_new(ptr @kai.str.18, i64 3)
  %f = getelementptr inbounds nuw %Named, ptr %tmp119, i32 0, i32 0
  store ptr %str120, ptr %f, align 8
  %f121 = getelementptr inbounds nuw %Named, ptr %tmp119, i32 0, i32 1
  store i32 7, ptr %f121, align 4
  %lit122 = load %Named, ptr %tmp119, align 8
  store %Named %lit122, ptr %n, align 8
  %field = getelementptr inbounds nuw %Named, ptr %n, i32 0, i32 0
  %field123 = load ptr, ptr %field, align 8
  call void @kai_retain(ptr %field123)
  store ptr %field123, ptr %label, align 8
  %place = getelementptr inbounds nuw %Named, ptr %n, i32 0, i32 1
  store i32 8, ptr %place, align 4
  %str124 = call ptr @kai_string_new(ptr @kai.str.19, i64 3)
  store ptr %str124, ptr %"$tmp125", align 8
  %tmp126 = load ptr, ptr %label, align 8
  %tmp127 = load ptr, ptr %"$tmp125", align 8
  %str.eq128 = call i8 @kai_string_eq(ptr %tmp126, ptr %tmp127)
  %str.eq.b129 = trunc i8 %str.eq128 to i1
  br i1 %str.eq.b129, label %and.rhs130, label %and.end131

and.rhs130:                                       ; preds = %arith.ok117
  %str132 = call ptr @kai_string_new(ptr @kai.str.20, i64 3)
  store ptr %str132, ptr %"$tmp133", align 8
  %field134 = getelementptr inbounds nuw %Named, ptr %n, i32 0, i32 0
  %field135 = load ptr, ptr %field134, align 8
  %tmp136 = load ptr, ptr %"$tmp133", align 8
  %str.eq137 = call i8 @kai_string_eq(ptr %field135, ptr %tmp136)
  %str.eq.b138 = trunc i8 %str.eq137 to i1
  %rel.hdr139 = load ptr, ptr %"$tmp133", align 8
  call void @kai_release(ptr %rel.hdr139)
  br label %and.end131

and.end131:                                       ; preds = %and.rhs130, %arith.ok117
  %and.result140 = phi i1 [ %str.eq.b129, %arith.ok117 ], [ %str.eq.b138, %and.rhs130 ]
  br i1 %and.result140, label %and.rhs141, label %and.end142

and.rhs141:                                       ; preds = %and.end131
  %field143 = getelementptr inbounds nuw %Named, ptr %n, i32 0, i32 1
  %field144 = load i32, ptr %field143, align 4
  %eq145 = icmp eq i32 %field144, 8
  br label %and.end142

and.end142:                                       ; preds = %and.rhs141, %and.end131
  %and.result146 = phi i1 [ %and.result140, %and.end131 ], [ %eq145, %and.rhs141 ]
  br i1 %and.result146, label %if.then147, label %if.end148

if.then147:                                       ; preds = %and.end142
  %old149 = load i32, ptr %score, align 4
  %ovf150 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old149, i32 10)
  %ovf.flag151 = extractvalue { i32, i1 } %ovf150, 1
  br i1 %ovf.flag151, label %panic152, label %arith.ok153

if.end148:                                        ; preds = %arith.ok153, %and.end142
  %tmp155 = load i32, ptr %score, align 4
  %rel.hdr156 = load ptr, ptr %"$tmp125", align 8
  call void @kai_release(ptr %rel.hdr156)
  %rel.hdr157 = load ptr, ptr %label, align 8
  call void @kai_release(ptr %rel.hdr157)
  call void @kai.release_Named(ptr %n)
  %rel.hdr158 = load ptr, ptr %nums, align 8
  call void @kai_release(ptr %rel.hdr158)
  %rel.hdr159 = load ptr, ptr %"$tmp39", align 8
  call void @kai_release(ptr %rel.hdr159)
  %rel.hdr160 = load ptr, ptr %words, align 8
  call void @kai_release(ptr %rel.hdr160)
  %rel.hdr161 = load ptr, ptr %out, align 8
  call void @kai_release(ptr %rel.hdr161)
  %rel.hdr162 = load ptr, ptr %lit, align 8
  call void @kai_release(ptr %rel.hdr162)
  ret i32 %tmp155

panic152:                                         ; preds = %if.then147
  call void @kai_panic(ptr @kai.panic.msg.21, i64 16, ptr @kai.src.file, i64 67, i64 9)
  unreachable

arith.ok153:                                      ; preds = %if.then147
  %add154 = extractvalue { i32, i1 } %ovf150, 0
  store i32 %add154, ptr %score, align 4
  br label %if.end148
}

declare void @kai_retain(ptr)

declare void @kai_release(ptr)

declare void @kai_panic(ptr, i64, ptr, i64, i64)

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.sadd.with.overflow.i32(i32, i32) #0

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

define void @kai.release_Named(ptr %0) {
entry:
  %fld = getelementptr inbounds nuw %Named, ptr %0, i32 0, i32 0
  %rel.hdr = load ptr, ptr %fld, align 8
  call void @kai_release(ptr %rel.hdr)
  ret void
}

attributes #0 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
