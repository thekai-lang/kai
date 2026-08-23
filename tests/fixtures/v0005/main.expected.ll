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
@kai.panic.msg.10 = private unnamed_addr constant [26 x i8] c"array index out of bounds\00", align 1
@kai.str.11 = private unnamed_addr constant [2 x i8] c"aa"
@kai.panic.msg.12 = private unnamed_addr constant [26 x i8] c"array index out of bounds\00", align 1
@kai.str.13 = private unnamed_addr constant [2 x i8] c"bb"
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
  %tmp = load ptr, ptr %s1, align 8
  call void @kai_retain(ptr %tmp)
  ret ptr %tmp
}

define void @set_first(ptr %arr) {
entry:
  %arr1 = alloca ptr, align 8
  store ptr %arr, ptr %arr1, align 8
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
  %label = alloca ptr, align 8
  %n = alloca %Named, align 8
  %tmp106 = alloca %Named, align 8
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
  %str4 = call ptr @kai_string_new(ptr @kai.str.2, i64 3)
  %str.eq5 = call i8 @kai_string_eq(ptr %tmp3, ptr %str4)
  %str.eq.b6 = trunc i8 %str.eq5 to i1
  br label %and.end

and.end:                                          ; preds = %and.rhs, %entry
  %and.result = phi i1 [ %str.eq.b, %entry ], [ %str.eq.b6, %and.rhs ]
  br i1 %and.result, label %and.rhs7, label %and.end8

and.rhs7:                                         ; preds = %and.end
  %str9 = call ptr @kai_string_new(ptr @kai.str.3, i64 3)
  %str10 = call ptr @kai_string_new(ptr @kai.str.4, i64 3)
  %str.eq11 = call i8 @kai_string_eq(ptr %str9, ptr %str10)
  %str.eq.b12 = trunc i8 %str.eq11 to i1
  %str.ne = xor i1 %str.eq.b12, true
  br label %and.end8

and.end8:                                         ; preds = %and.rhs7, %and.end
  %and.result13 = phi i1 [ %and.result, %and.end ], [ %str.ne, %and.rhs7 ]
  br i1 %and.result13, label %if.then, label %if.end

if.then:                                          ; preds = %and.end8
  %old = load i32, ptr %score, align 4
  %ovf = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old, i32 1)
  %ovf.flag = extractvalue { i32, i1 } %ovf, 1
  br i1 %ovf.flag, label %panic, label %arith.ok

if.end:                                           ; preds = %arith.ok, %and.end8
  %arr = call ptr @kai_array_new(i64 2, i64 ptrtoint (ptr getelementptr (ptr, ptr null, i32 1) to i64), ptr @kai.dtor.elems_string)
  %arr.elems.p = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %arr, i32 0, i32 3
  %arr.elems = load ptr, ptr %arr.elems.p, align 8
  %str14 = call ptr @kai_string_new(ptr @kai.str.6, i64 2)
  %arr.slot = getelementptr inbounds ptr, ptr %arr.elems, i64 0
  store ptr %str14, ptr %arr.slot, align 8
  %str15 = call ptr @kai_string_new(ptr @kai.str.7, i64 2)
  %arr.slot16 = getelementptr inbounds ptr, ptr %arr.elems, i64 1
  store ptr %str15, ptr %arr.slot16, align 8
  store ptr %arr, ptr %words, align 8
  %arr.hdr = load ptr, ptr %words, align 8
  %arr.len.p = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %arr.hdr, i32 0, i32 1
  %arr.len = load i64, ptr %arr.len.p, align 4
  %bnd.high = icmp slt i64 0, %arr.len
  %bnd.ok = and i1 true, %bnd.high
  %bnd.bad = xor i1 %bnd.ok, true
  br i1 %bnd.bad, label %panic17, label %in.bounds

panic:                                            ; preds = %if.then
  call void @kai_panic(ptr @kai.panic.msg.5, i64 16, ptr @kai.src.file, i64 39, i64 9)
  unreachable

arith.ok:                                         ; preds = %if.then
  %add = extractvalue { i32, i1 } %ovf, 0
  store i32 %add, ptr %score, align 4
  br label %if.end

panic17:                                          ; preds = %if.end
  call void @kai_panic(ptr @kai.panic.msg.8, i64 25, ptr @kai.src.file, i64 44, i64 11)
  unreachable

in.bounds:                                        ; preds = %if.end
  %arr.elems.p18 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %arr.hdr, i32 0, i32 3
  %arr.elems19 = load ptr, ptr %arr.elems.p18, align 8
  %place.elem = getelementptr inbounds ptr, ptr %arr.elems19, i64 0
  %tmp20 = load ptr, ptr %words, align 8
  %arr.len.p21 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp20, i32 0, i32 1
  %arr.len22 = load i64, ptr %arr.len.p21, align 4
  %bnd.high23 = icmp slt i64 0, %arr.len22
  %bnd.ok24 = and i1 true, %bnd.high23
  %bnd.bad25 = xor i1 %bnd.ok24, true
  br i1 %bnd.bad25, label %panic26, label %in.bounds27

panic26:                                          ; preds = %in.bounds
  call void @kai_panic(ptr @kai.panic.msg.9, i64 25, ptr @kai.src.file, i64 44, i64 16)
  unreachable

in.bounds27:                                      ; preds = %in.bounds
  %arr.elems.p28 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp20, i32 0, i32 3
  %arr.elems29 = load ptr, ptr %arr.elems.p28, align 8
  %elem.slot = getelementptr inbounds ptr, ptr %arr.elems29, i64 0
  %elem = load ptr, ptr %elem.slot, align 8
  call void @kai_retain(ptr %elem)
  %rel.hdr = load ptr, ptr %place.elem, align 8
  call void @kai_release(ptr %rel.hdr)
  store ptr %elem, ptr %place.elem, align 8
  %tmp30 = load ptr, ptr %words, align 8
  %arr.len.p31 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp30, i32 0, i32 1
  %arr.len32 = load i64, ptr %arr.len.p31, align 4
  %bnd.high33 = icmp slt i64 0, %arr.len32
  %bnd.ok34 = and i1 true, %bnd.high33
  %bnd.bad35 = xor i1 %bnd.ok34, true
  br i1 %bnd.bad35, label %panic36, label %in.bounds37

panic36:                                          ; preds = %in.bounds27
  call void @kai_panic(ptr @kai.panic.msg.10, i64 25, ptr @kai.src.file, i64 45, i64 8)
  unreachable

in.bounds37:                                      ; preds = %in.bounds27
  %arr.elems.p38 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp30, i32 0, i32 3
  %arr.elems39 = load ptr, ptr %arr.elems.p38, align 8
  %elem.slot40 = getelementptr inbounds ptr, ptr %arr.elems39, i64 0
  %elem41 = load ptr, ptr %elem.slot40, align 8
  %str42 = call ptr @kai_string_new(ptr @kai.str.11, i64 2)
  %str.eq43 = call i8 @kai_string_eq(ptr %elem41, ptr %str42)
  %str.eq.b44 = trunc i8 %str.eq43 to i1
  br i1 %str.eq.b44, label %and.rhs45, label %and.end46

and.rhs45:                                        ; preds = %in.bounds37
  %tmp47 = load ptr, ptr %words, align 8
  %arr.len.p48 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp47, i32 0, i32 1
  %arr.len49 = load i64, ptr %arr.len.p48, align 4
  %bnd.high50 = icmp slt i64 1, %arr.len49
  %bnd.ok51 = and i1 true, %bnd.high50
  %bnd.bad52 = xor i1 %bnd.ok51, true
  br i1 %bnd.bad52, label %panic53, label %in.bounds54

and.end46:                                        ; preds = %in.bounds54, %in.bounds37
  %and.result62 = phi i1 [ %str.eq.b44, %in.bounds37 ], [ %str.eq.b61, %in.bounds54 ]
  br i1 %and.result62, label %if.then63, label %if.end64

panic53:                                          ; preds = %and.rhs45
  call void @kai_panic(ptr @kai.panic.msg.12, i64 25, ptr @kai.src.file, i64 45, i64 28)
  unreachable

in.bounds54:                                      ; preds = %and.rhs45
  %arr.elems.p55 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp47, i32 0, i32 3
  %arr.elems56 = load ptr, ptr %arr.elems.p55, align 8
  %elem.slot57 = getelementptr inbounds ptr, ptr %arr.elems56, i64 1
  %elem58 = load ptr, ptr %elem.slot57, align 8
  %str59 = call ptr @kai_string_new(ptr @kai.str.13, i64 2)
  %str.eq60 = call i8 @kai_string_eq(ptr %elem58, ptr %str59)
  %str.eq.b61 = trunc i8 %str.eq60 to i1
  br label %and.end46

if.then63:                                        ; preds = %and.end46
  %old65 = load i32, ptr %score, align 4
  %ovf66 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old65, i32 2)
  %ovf.flag67 = extractvalue { i32, i1 } %ovf66, 1
  br i1 %ovf.flag67, label %panic68, label %arith.ok69

if.end64:                                         ; preds = %arith.ok69, %and.end46
  %arr71 = call ptr @kai_array_new(i64 3, i64 ptrtoint (ptr getelementptr (i32, ptr null, i32 1) to i64), ptr null)
  %arr.elems.p72 = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %arr71, i32 0, i32 3
  %arr.elems73 = load ptr, ptr %arr.elems.p72, align 8
  %arr.slot74 = getelementptr inbounds i32, ptr %arr.elems73, i64 0
  store i32 1, ptr %arr.slot74, align 4
  %arr.slot75 = getelementptr inbounds i32, ptr %arr.elems73, i64 1
  store i32 2, ptr %arr.slot75, align 4
  %arr.slot76 = getelementptr inbounds i32, ptr %arr.elems73, i64 2
  store i32 3, ptr %arr.slot76, align 4
  store ptr %arr71, ptr %nums, align 8
  %tmp77 = load ptr, ptr %nums, align 8
  call void @set_first(ptr %tmp77)
  %tmp78 = load ptr, ptr %nums, align 8
  %arr.len.p79 = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %tmp78, i32 0, i32 1
  %arr.len80 = load i64, ptr %arr.len.p79, align 4
  %bnd.high81 = icmp slt i64 0, %arr.len80
  %bnd.ok82 = and i1 true, %bnd.high81
  %bnd.bad83 = xor i1 %bnd.ok82, true
  br i1 %bnd.bad83, label %panic84, label %in.bounds85

panic68:                                          ; preds = %if.then63
  call void @kai_panic(ptr @kai.panic.msg.14, i64 16, ptr @kai.src.file, i64 46, i64 9)
  unreachable

arith.ok69:                                       ; preds = %if.then63
  %add70 = extractvalue { i32, i1 } %ovf66, 0
  store i32 %add70, ptr %score, align 4
  br label %if.end64

panic84:                                          ; preds = %if.end64
  call void @kai_panic(ptr @kai.panic.msg.15, i64 25, ptr @kai.src.file, i64 53, i64 8)
  unreachable

in.bounds85:                                      ; preds = %if.end64
  %arr.elems.p86 = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %tmp78, i32 0, i32 3
  %arr.elems87 = load ptr, ptr %arr.elems.p86, align 8
  %elem.slot88 = getelementptr inbounds i32, ptr %arr.elems87, i64 0
  %elem89 = load i32, ptr %elem.slot88, align 4
  %eq = icmp eq i32 %elem89, 42
  br i1 %eq, label %if.then90, label %if.end91

if.then90:                                        ; preds = %in.bounds85
  %old92 = load i32, ptr %score, align 4
  %ovf93 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old92, i32 4)
  %ovf.flag94 = extractvalue { i32, i1 } %ovf93, 1
  br i1 %ovf.flag94, label %panic95, label %arith.ok96

if.end91:                                         ; preds = %arith.ok96, %in.bounds85
  %tmp98 = load ptr, ptr %nums, align 8
  %call99 = call i32 @sum(ptr %tmp98)
  %old100 = load i32, ptr %score, align 4
  %ovf101 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old100, i32 %call99)
  %ovf.flag102 = extractvalue { i32, i1 } %ovf101, 1
  br i1 %ovf.flag102, label %panic103, label %arith.ok104

panic95:                                          ; preds = %if.then90
  call void @kai_panic(ptr @kai.panic.msg.16, i64 16, ptr @kai.src.file, i64 54, i64 9)
  unreachable

arith.ok96:                                       ; preds = %if.then90
  %add97 = extractvalue { i32, i1 } %ovf93, 0
  store i32 %add97, ptr %score, align 4
  br label %if.end91

panic103:                                         ; preds = %if.end91
  call void @kai_panic(ptr @kai.panic.msg.17, i64 16, ptr @kai.src.file, i64 59, i64 5)
  unreachable

arith.ok104:                                      ; preds = %if.end91
  %add105 = extractvalue { i32, i1 } %ovf101, 0
  store i32 %add105, ptr %score, align 4
  %str107 = call ptr @kai_string_new(ptr @kai.str.18, i64 3)
  %f = getelementptr inbounds nuw %Named, ptr %tmp106, i32 0, i32 0
  store ptr %str107, ptr %f, align 8
  %f108 = getelementptr inbounds nuw %Named, ptr %tmp106, i32 0, i32 1
  store i32 7, ptr %f108, align 4
  %lit109 = load %Named, ptr %tmp106, align 8
  store %Named %lit109, ptr %n, align 8
  %field = getelementptr inbounds nuw %Named, ptr %n, i32 0, i32 0
  %field110 = load ptr, ptr %field, align 8
  call void @kai_retain(ptr %field110)
  store ptr %field110, ptr %label, align 8
  %place = getelementptr inbounds nuw %Named, ptr %n, i32 0, i32 1
  store i32 8, ptr %place, align 4
  %tmp111 = load ptr, ptr %label, align 8
  %str112 = call ptr @kai_string_new(ptr @kai.str.19, i64 3)
  %str.eq113 = call i8 @kai_string_eq(ptr %tmp111, ptr %str112)
  %str.eq.b114 = trunc i8 %str.eq113 to i1
  br i1 %str.eq.b114, label %and.rhs115, label %and.end116

and.rhs115:                                       ; preds = %arith.ok104
  %field117 = getelementptr inbounds nuw %Named, ptr %n, i32 0, i32 0
  %field118 = load ptr, ptr %field117, align 8
  %str119 = call ptr @kai_string_new(ptr @kai.str.20, i64 3)
  %str.eq120 = call i8 @kai_string_eq(ptr %field118, ptr %str119)
  %str.eq.b121 = trunc i8 %str.eq120 to i1
  br label %and.end116

and.end116:                                       ; preds = %and.rhs115, %arith.ok104
  %and.result122 = phi i1 [ %str.eq.b114, %arith.ok104 ], [ %str.eq.b121, %and.rhs115 ]
  br i1 %and.result122, label %and.rhs123, label %and.end124

and.rhs123:                                       ; preds = %and.end116
  %field125 = getelementptr inbounds nuw %Named, ptr %n, i32 0, i32 1
  %field126 = load i32, ptr %field125, align 4
  %eq127 = icmp eq i32 %field126, 8
  br label %and.end124

and.end124:                                       ; preds = %and.rhs123, %and.end116
  %and.result128 = phi i1 [ %and.result122, %and.end116 ], [ %eq127, %and.rhs123 ]
  br i1 %and.result128, label %if.then129, label %if.end130

if.then129:                                       ; preds = %and.end124
  %old131 = load i32, ptr %score, align 4
  %ovf132 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old131, i32 10)
  %ovf.flag133 = extractvalue { i32, i1 } %ovf132, 1
  br i1 %ovf.flag133, label %panic134, label %arith.ok135

if.end130:                                        ; preds = %arith.ok135, %and.end124
  %tmp137 = load i32, ptr %score, align 4
  %rel.hdr138 = load ptr, ptr %label, align 8
  call void @kai_release(ptr %rel.hdr138)
  call void @kai.release_Named(ptr %n)
  %rel.hdr139 = load ptr, ptr %nums, align 8
  call void @kai_release(ptr %rel.hdr139)
  %rel.hdr140 = load ptr, ptr %words, align 8
  call void @kai_release(ptr %rel.hdr140)
  %rel.hdr141 = load ptr, ptr %out, align 8
  call void @kai_release(ptr %rel.hdr141)
  %rel.hdr142 = load ptr, ptr %lit, align 8
  call void @kai_release(ptr %rel.hdr142)
  ret i32 %tmp137

panic134:                                         ; preds = %if.then129
  call void @kai_panic(ptr @kai.panic.msg.21, i64 16, ptr @kai.src.file, i64 67, i64 9)
  unreachable

arith.ok135:                                      ; preds = %if.then129
  %add136 = extractvalue { i32, i1 } %ovf132, 0
  store i32 %add136, ptr %score, align 4
  br label %if.end130
}

declare void @kai_retain(ptr)

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

declare void @kai_release(ptr)

define void @kai.release_Named(ptr %0) {
entry:
  %fld = getelementptr inbounds nuw %Named, ptr %0, i32 0, i32 0
  %rel.hdr = load ptr, ptr %fld, align 8
  call void @kai_release(ptr %rel.hdr)
  ret void
}

attributes #0 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
