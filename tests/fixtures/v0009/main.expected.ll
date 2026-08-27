; ModuleID = 'kai_module'
source_filename = "kai_module"

%Point = type { i32, ptr }
%"KaiArray.\22i32\22" = type { i64, i64, i64, ptr, ptr }
%"KaiArray.\22ptr\22" = type { i64, i64, i64, ptr, ptr }

@kai.panic.msg = private unnamed_addr constant [26 x i8] c"array index out of bounds\00", align 1
@kai.src.file = private unnamed_addr constant [9 x i8] c"main.kai\00", align 1
@kai.panic.msg.1 = private unnamed_addr constant [26 x i8] c"array index out of bounds\00", align 1
@kai.str = private unnamed_addr constant [5 x i8] c"moved"
@kai.panic.msg.2 = private unnamed_addr constant [26 x i8] c"array index out of bounds\00", align 1
@kai.panic.msg.3 = private unnamed_addr constant [26 x i8] c"array index out of bounds\00", align 1
@kai.panic.msg.4 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.panic.msg.5 = private unnamed_addr constant [26 x i8] c"array index out of bounds\00", align 1
@kai.str.6 = private unnamed_addr constant [1 x i8] c"a"
@kai.str.7 = private unnamed_addr constant [1 x i8] c"b"
@kai.str.8 = private unnamed_addr constant [1 x i8] c"p"
@kai.panic.msg.9 = private unnamed_addr constant [26 x i8] c"array index out of bounds\00", align 1
@kai.panic.msg.10 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.panic.msg.11 = private unnamed_addr constant [26 x i8] c"array index out of bounds\00", align 1
@kai.panic.msg.12 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.str.13 = private unnamed_addr constant [5 x i8] c"moved"
@kai.panic.msg.14 = private unnamed_addr constant [26 x i8] c"array index out of bounds\00", align 1
@kai.panic.msg.15 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.str.16 = private unnamed_addr constant [1 x i8] c"p"
@kai.panic.msg.17 = private unnamed_addr constant [26 x i8] c"array index out of bounds\00", align 1
@kai.panic.msg.18 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1
@kai.panic.msg.19 = private unnamed_addr constant [17 x i8] c"integer overflow\00", align 1

define void @rework(ptr %ns, ptr %words, %Point %pt) {
entry:
  %rev.snap55 = alloca ptr, align 8
  %rev.snap31 = alloca i32, align 4
  %rev.snap18 = alloca i32, align 4
  %rev.snap16 = alloca ptr, align 8
  %rev.snap = alloca i32, align 4
  %pt4 = alloca %Point, align 8
  %words2 = alloca ptr, align 8
  %ns1 = alloca ptr, align 8
  call void @kai_reversible_enter()
  store ptr %ns, ptr %ns1, align 8
  %ret.hdr = load ptr, ptr %ns1, align 8
  call void @kai_retain(ptr %ret.hdr)
  store ptr %words, ptr %words2, align 8
  %ret.hdr3 = load ptr, ptr %words2, align 8
  call void @kai_retain(ptr %ret.hdr3)
  store %Point %pt, ptr %pt4, align 8
  call void @kai.retain_Point(ptr %pt4)
  %copied = load %Point, ptr %pt4, align 8
  %arr.hdr = load ptr, ptr %ns1, align 8
  %arr.len.p = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %arr.hdr, i32 0, i32 1
  %arr.len = load i64, ptr %arr.len.p, align 4
  %bnd.high = icmp slt i64 0, %arr.len
  %bnd.ok = and i1 true, %bnd.high
  %bnd.bad = xor i1 %bnd.ok, true
  br i1 %bnd.bad, label %panic, label %in.bounds

panic:                                            ; preds = %entry
  call void @kai_reversible_unwind()
  call void @kai_panic(ptr @kai.panic.msg, i64 25, ptr @kai.src.file, i64 8, i64 8)
  unreachable

in.bounds:                                        ; preds = %entry
  %arr.elems.p = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %arr.hdr, i32 0, i32 3
  %arr.elems = load ptr, ptr %arr.elems.p, align 8
  %place.elem = getelementptr inbounds i32, ptr %arr.elems, i64 0
  %retained = load i32, ptr %place.elem, align 4
  store i32 %retained, ptr %rev.snap, align 4
  call void @kai_reversible_push(ptr %place.elem, ptr %rev.snap, i64 ptrtoint (ptr getelementptr (i32, ptr null, i32 1) to i64), ptr null)
  store i32 9, ptr %place.elem, align 4
  %arr.hdr5 = load ptr, ptr %words2, align 8
  %arr.len.p6 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %arr.hdr5, i32 0, i32 1
  %arr.len7 = load i64, ptr %arr.len.p6, align 4
  %bnd.high8 = icmp slt i64 0, %arr.len7
  %bnd.ok9 = and i1 true, %bnd.high8
  %bnd.bad10 = xor i1 %bnd.ok9, true
  br i1 %bnd.bad10, label %panic11, label %in.bounds12

panic11:                                          ; preds = %in.bounds
  call void @kai_reversible_unwind()
  call void @kai_panic(ptr @kai.panic.msg.1, i64 25, ptr @kai.src.file, i64 9, i64 11)
  unreachable

in.bounds12:                                      ; preds = %in.bounds
  %arr.elems.p13 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %arr.hdr5, i32 0, i32 3
  %arr.elems14 = load ptr, ptr %arr.elems.p13, align 8
  %place.elem15 = getelementptr inbounds ptr, ptr %arr.elems14, i64 0
  %ret.hdr17 = load ptr, ptr %place.elem15, align 8
  call void @kai_retain(ptr %ret.hdr17)
  store ptr %ret.hdr17, ptr %rev.snap16, align 8
  call void @kai_reversible_push(ptr %place.elem15, ptr %rev.snap16, i64 ptrtoint (ptr getelementptr (ptr, ptr null, i32 1) to i64), ptr @kai.snapREL_string)
  %str = call ptr @kai_string_new(ptr @kai.str, i64 5)
  %rel.hdr = load ptr, ptr %place.elem15, align 8
  call void @kai_release(ptr %rel.hdr)
  store ptr %str, ptr %place.elem15, align 8
  %place = getelementptr inbounds nuw %Point, ptr %pt4, i32 0, i32 0
  %retained19 = load i32, ptr %place, align 4
  store i32 %retained19, ptr %rev.snap18, align 4
  call void @kai_reversible_push(ptr %place, ptr %rev.snap18, i64 ptrtoint (ptr getelementptr (i32, ptr null, i32 1) to i64), ptr null)
  store i32 41, ptr %place, align 4
  %arr.hdr20 = load ptr, ptr %ns1, align 8
  %arr.len.p21 = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %arr.hdr20, i32 0, i32 1
  %arr.len22 = load i64, ptr %arr.len.p21, align 4
  %bnd.high23 = icmp slt i64 1, %arr.len22
  %bnd.ok24 = and i1 true, %bnd.high23
  %bnd.bad25 = xor i1 %bnd.ok24, true
  br i1 %bnd.bad25, label %panic26, label %in.bounds27

panic26:                                          ; preds = %in.bounds12
  call void @kai_reversible_unwind()
  call void @kai_panic(ptr @kai.panic.msg.2, i64 25, ptr @kai.src.file, i64 11, i64 8)
  unreachable

in.bounds27:                                      ; preds = %in.bounds12
  %arr.elems.p28 = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %arr.hdr20, i32 0, i32 3
  %arr.elems29 = load ptr, ptr %arr.elems.p28, align 8
  %place.elem30 = getelementptr inbounds i32, ptr %arr.elems29, i64 1
  %retained32 = load i32, ptr %place.elem30, align 4
  store i32 %retained32, ptr %rev.snap31, align 4
  call void @kai_reversible_push(ptr %place.elem30, ptr %rev.snap31, i64 ptrtoint (ptr getelementptr (i32, ptr null, i32 1) to i64), ptr null)
  %tmp = load ptr, ptr %ns1, align 8
  %arr.len.p33 = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %tmp, i32 0, i32 1
  %arr.len34 = load i64, ptr %arr.len.p33, align 4
  %bnd.high35 = icmp slt i64 0, %arr.len34
  %bnd.ok36 = and i1 true, %bnd.high35
  %bnd.bad37 = xor i1 %bnd.ok36, true
  br i1 %bnd.bad37, label %panic38, label %in.bounds39

panic38:                                          ; preds = %in.bounds27
  call void @kai_reversible_unwind()
  call void @kai_panic(ptr @kai.panic.msg.3, i64 25, ptr @kai.src.file, i64 11, i64 13)
  unreachable

in.bounds39:                                      ; preds = %in.bounds27
  %arr.elems.p40 = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %tmp, i32 0, i32 3
  %arr.elems41 = load ptr, ptr %arr.elems.p40, align 8
  %elem.slot = getelementptr inbounds i32, ptr %arr.elems41, i64 0
  %elem = load i32, ptr %elem.slot, align 4
  %field = getelementptr inbounds nuw %Point, ptr %pt4, i32 0, i32 0
  %field42 = load i32, ptr %field, align 4
  %ovf = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %elem, i32 %field42)
  %ovf.flag = extractvalue { i32, i1 } %ovf, 1
  br i1 %ovf.flag, label %panic43, label %arith.ok

panic43:                                          ; preds = %in.bounds39
  call void @kai_reversible_unwind()
  call void @kai_panic(ptr @kai.panic.msg.4, i64 16, ptr @kai.src.file, i64 11, i64 13)
  unreachable

arith.ok:                                         ; preds = %in.bounds39
  %add = extractvalue { i32, i1 } %ovf, 0
  store i32 %add, ptr %place.elem30, align 4
  %arr.hdr44 = load ptr, ptr %words2, align 8
  %arr.len.p45 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %arr.hdr44, i32 0, i32 1
  %arr.len46 = load i64, ptr %arr.len.p45, align 4
  %bnd.high47 = icmp slt i64 1, %arr.len46
  %bnd.ok48 = and i1 true, %bnd.high47
  %bnd.bad49 = xor i1 %bnd.ok48, true
  br i1 %bnd.bad49, label %panic50, label %in.bounds51

panic50:                                          ; preds = %arith.ok
  call void @kai_reversible_unwind()
  call void @kai_panic(ptr @kai.panic.msg.5, i64 25, ptr @kai.src.file, i64 12, i64 11)
  unreachable

in.bounds51:                                      ; preds = %arith.ok
  %arr.elems.p52 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %arr.hdr44, i32 0, i32 3
  %arr.elems53 = load ptr, ptr %arr.elems.p52, align 8
  %place.elem54 = getelementptr inbounds ptr, ptr %arr.elems53, i64 1
  %ret.hdr56 = load ptr, ptr %place.elem54, align 8
  call void @kai_retain(ptr %ret.hdr56)
  store ptr %ret.hdr56, ptr %rev.snap55, align 8
  call void @kai_reversible_push(ptr %place.elem54, ptr %rev.snap55, i64 ptrtoint (ptr getelementptr (ptr, ptr null, i32 1) to i64), ptr @kai.snapREL_string)
  %field57 = getelementptr inbounds nuw %Point, ptr %pt4, i32 0, i32 1
  %field58 = load ptr, ptr %field57, align 8
  call void @kai_retain(ptr %field58)
  %rel.hdr59 = load ptr, ptr %place.elem54, align 8
  call void @kai_release(ptr %rel.hdr59)
  store ptr %field58, ptr %place.elem54, align 8
  call void @kai.release_Point(ptr %pt4)
  %rel.hdr60 = load ptr, ptr %words2, align 8
  call void @kai_release(ptr %rel.hdr60)
  %rel.hdr61 = load ptr, ptr %ns1, align 8
  call void @kai_release(ptr %rel.hdr61)
  call void @kai_reversible_commit()
  ret void
}

define i32 @main() {
entry:
  %"$tmp61" = alloca ptr, align 8
  %"$tmp" = alloca ptr, align 8
  %total = alloca i32, align 4
  %pt = alloca %Point, align 8
  %tmp = alloca %Point, align 8
  %words = alloca ptr, align 8
  %ns = alloca ptr, align 8
  %arr = call ptr @kai_array_new(i64 2, i64 ptrtoint (ptr getelementptr (i32, ptr null, i32 1) to i64), ptr null)
  %arr.elems.p = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %arr, i32 0, i32 3
  %arr.elems = load ptr, ptr %arr.elems.p, align 8
  %arr.slot = getelementptr inbounds i32, ptr %arr.elems, i64 0
  store i32 0, ptr %arr.slot, align 4
  %arr.slot1 = getelementptr inbounds i32, ptr %arr.elems, i64 1
  store i32 0, ptr %arr.slot1, align 4
  store ptr %arr, ptr %ns, align 8
  %arr2 = call ptr @kai_array_new(i64 2, i64 ptrtoint (ptr getelementptr (ptr, ptr null, i32 1) to i64), ptr @kai.dtor.elems_string)
  %arr.elems.p3 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %arr2, i32 0, i32 3
  %arr.elems4 = load ptr, ptr %arr.elems.p3, align 8
  %str = call ptr @kai_string_new(ptr @kai.str.6, i64 1)
  %arr.slot5 = getelementptr inbounds ptr, ptr %arr.elems4, i64 0
  store ptr %str, ptr %arr.slot5, align 8
  %str6 = call ptr @kai_string_new(ptr @kai.str.7, i64 1)
  %arr.slot7 = getelementptr inbounds ptr, ptr %arr.elems4, i64 1
  store ptr %str6, ptr %arr.slot7, align 8
  store ptr %arr2, ptr %words, align 8
  %f = getelementptr inbounds nuw %Point, ptr %tmp, i32 0, i32 0
  store i32 0, ptr %f, align 4
  %str8 = call ptr @kai_string_new(ptr @kai.str.8, i64 1)
  %f9 = getelementptr inbounds nuw %Point, ptr %tmp, i32 0, i32 1
  store ptr %str8, ptr %f9, align 8
  %lit = load %Point, ptr %tmp, align 8
  store %Point %lit, ptr %pt, align 8
  %tmp10 = load ptr, ptr %ns, align 8
  %tmp11 = load ptr, ptr %words, align 8
  %tmp12 = load %Point, ptr %pt, align 8
  call void @rework(ptr %tmp10, ptr %tmp11, %Point %tmp12)
  store i32 0, ptr %total, align 4
  %tmp13 = load ptr, ptr %ns, align 8
  %arr.len.p = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %tmp13, i32 0, i32 1
  %arr.len = load i64, ptr %arr.len.p, align 4
  %bnd.high = icmp slt i64 0, %arr.len
  %bnd.ok = and i1 true, %bnd.high
  %bnd.bad = xor i1 %bnd.ok, true
  br i1 %bnd.bad, label %panic, label %in.bounds

panic:                                            ; preds = %entry
  call void @kai_panic(ptr @kai.panic.msg.9, i64 25, ptr @kai.src.file, i64 21, i64 8)
  unreachable

in.bounds:                                        ; preds = %entry
  %arr.elems.p14 = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %tmp13, i32 0, i32 3
  %arr.elems15 = load ptr, ptr %arr.elems.p14, align 8
  %elem.slot = getelementptr inbounds i32, ptr %arr.elems15, i64 0
  %elem = load i32, ptr %elem.slot, align 4
  %eq = icmp eq i32 %elem, 9
  br i1 %eq, label %if.then, label %if.end

if.then:                                          ; preds = %in.bounds
  %old = load i32, ptr %total, align 4
  %ovf = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old, i32 1)
  %ovf.flag = extractvalue { i32, i1 } %ovf, 1
  br i1 %ovf.flag, label %panic16, label %arith.ok

if.end:                                           ; preds = %arith.ok, %in.bounds
  %tmp17 = load ptr, ptr %ns, align 8
  %arr.len.p18 = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %tmp17, i32 0, i32 1
  %arr.len19 = load i64, ptr %arr.len.p18, align 4
  %bnd.high20 = icmp slt i64 1, %arr.len19
  %bnd.ok21 = and i1 true, %bnd.high20
  %bnd.bad22 = xor i1 %bnd.ok21, true
  br i1 %bnd.bad22, label %panic23, label %in.bounds24

panic16:                                          ; preds = %if.then
  call void @kai_panic(ptr @kai.panic.msg.10, i64 16, ptr @kai.src.file, i64 21, i64 21)
  unreachable

arith.ok:                                         ; preds = %if.then
  %add = extractvalue { i32, i1 } %ovf, 0
  store i32 %add, ptr %total, align 4
  br label %if.end

panic23:                                          ; preds = %if.end
  call void @kai_panic(ptr @kai.panic.msg.11, i64 25, ptr @kai.src.file, i64 22, i64 8)
  unreachable

in.bounds24:                                      ; preds = %if.end
  %arr.elems.p25 = getelementptr inbounds nuw %"KaiArray.\22i32\22", ptr %tmp17, i32 0, i32 3
  %arr.elems26 = load ptr, ptr %arr.elems.p25, align 8
  %elem.slot27 = getelementptr inbounds i32, ptr %arr.elems26, i64 1
  %elem28 = load i32, ptr %elem.slot27, align 4
  %eq29 = icmp eq i32 %elem28, 50
  br i1 %eq29, label %if.then30, label %if.end31

if.then30:                                        ; preds = %in.bounds24
  %old32 = load i32, ptr %total, align 4
  %ovf33 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old32, i32 2)
  %ovf.flag34 = extractvalue { i32, i1 } %ovf33, 1
  br i1 %ovf.flag34, label %panic35, label %arith.ok36

if.end31:                                         ; preds = %arith.ok36, %in.bounds24
  %str38 = call ptr @kai_string_new(ptr @kai.str.13, i64 5)
  store ptr %str38, ptr %"$tmp", align 8
  %tmp39 = load ptr, ptr %words, align 8
  %arr.len.p40 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp39, i32 0, i32 1
  %arr.len41 = load i64, ptr %arr.len.p40, align 4
  %bnd.high42 = icmp slt i64 0, %arr.len41
  %bnd.ok43 = and i1 true, %bnd.high42
  %bnd.bad44 = xor i1 %bnd.ok43, true
  br i1 %bnd.bad44, label %panic45, label %in.bounds46

panic35:                                          ; preds = %if.then30
  call void @kai_panic(ptr @kai.panic.msg.12, i64 16, ptr @kai.src.file, i64 22, i64 22)
  unreachable

arith.ok36:                                       ; preds = %if.then30
  %add37 = extractvalue { i32, i1 } %ovf33, 0
  store i32 %add37, ptr %total, align 4
  br label %if.end31

panic45:                                          ; preds = %if.end31
  call void @kai_panic(ptr @kai.panic.msg.14, i64 25, ptr @kai.src.file, i64 23, i64 8)
  unreachable

in.bounds46:                                      ; preds = %if.end31
  %arr.elems.p47 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp39, i32 0, i32 3
  %arr.elems48 = load ptr, ptr %arr.elems.p47, align 8
  %elem.slot49 = getelementptr inbounds ptr, ptr %arr.elems48, i64 0
  %elem50 = load ptr, ptr %elem.slot49, align 8
  %tmp51 = load ptr, ptr %"$tmp", align 8
  %str.eq = call i8 @kai_string_eq(ptr %elem50, ptr %tmp51)
  %str.eq.b = trunc i8 %str.eq to i1
  br i1 %str.eq.b, label %if.then52, label %if.end53

if.then52:                                        ; preds = %in.bounds46
  %old54 = load i32, ptr %total, align 4
  %ovf55 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old54, i32 4)
  %ovf.flag56 = extractvalue { i32, i1 } %ovf55, 1
  br i1 %ovf.flag56, label %panic57, label %arith.ok58

if.end53:                                         ; preds = %arith.ok58, %in.bounds46
  %str60 = call ptr @kai_string_new(ptr @kai.str.16, i64 1)
  store ptr %str60, ptr %"$tmp61", align 8
  %tmp62 = load ptr, ptr %words, align 8
  %arr.len.p63 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp62, i32 0, i32 1
  %arr.len64 = load i64, ptr %arr.len.p63, align 4
  %bnd.high65 = icmp slt i64 1, %arr.len64
  %bnd.ok66 = and i1 true, %bnd.high65
  %bnd.bad67 = xor i1 %bnd.ok66, true
  br i1 %bnd.bad67, label %panic68, label %in.bounds69

panic57:                                          ; preds = %if.then52
  call void @kai_panic(ptr @kai.panic.msg.15, i64 16, ptr @kai.src.file, i64 23, i64 30)
  unreachable

arith.ok58:                                       ; preds = %if.then52
  %add59 = extractvalue { i32, i1 } %ovf55, 0
  store i32 %add59, ptr %total, align 4
  br label %if.end53

panic68:                                          ; preds = %if.end53
  call void @kai_panic(ptr @kai.panic.msg.17, i64 25, ptr @kai.src.file, i64 24, i64 8)
  unreachable

in.bounds69:                                      ; preds = %if.end53
  %arr.elems.p70 = getelementptr inbounds nuw %"KaiArray.\22ptr\22", ptr %tmp62, i32 0, i32 3
  %arr.elems71 = load ptr, ptr %arr.elems.p70, align 8
  %elem.slot72 = getelementptr inbounds ptr, ptr %arr.elems71, i64 1
  %elem73 = load ptr, ptr %elem.slot72, align 8
  %tmp74 = load ptr, ptr %"$tmp61", align 8
  %str.eq75 = call i8 @kai_string_eq(ptr %elem73, ptr %tmp74)
  %str.eq.b76 = trunc i8 %str.eq75 to i1
  br i1 %str.eq.b76, label %if.then77, label %if.end78

if.then77:                                        ; preds = %in.bounds69
  %old79 = load i32, ptr %total, align 4
  %ovf80 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old79, i32 8)
  %ovf.flag81 = extractvalue { i32, i1 } %ovf80, 1
  br i1 %ovf.flag81, label %panic82, label %arith.ok83

if.end78:                                         ; preds = %arith.ok83, %in.bounds69
  %field = getelementptr inbounds nuw %Point, ptr %pt, i32 0, i32 0
  %field85 = load i32, ptr %field, align 4
  %eq86 = icmp eq i32 %field85, 0
  br i1 %eq86, label %if.then87, label %if.end88

panic82:                                          ; preds = %if.then77
  call void @kai_panic(ptr @kai.panic.msg.18, i64 16, ptr @kai.src.file, i64 24, i64 26)
  unreachable

arith.ok83:                                       ; preds = %if.then77
  %add84 = extractvalue { i32, i1 } %ovf80, 0
  store i32 %add84, ptr %total, align 4
  br label %if.end78

if.then87:                                        ; preds = %if.end78
  %old89 = load i32, ptr %total, align 4
  %ovf90 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %old89, i32 16)
  %ovf.flag91 = extractvalue { i32, i1 } %ovf90, 1
  br i1 %ovf.flag91, label %panic92, label %arith.ok93

if.end88:                                         ; preds = %arith.ok93, %if.end78
  %tmp95 = load i32, ptr %total, align 4
  %rel.hdr = load ptr, ptr %"$tmp61", align 8
  call void @kai_release(ptr %rel.hdr)
  %rel.hdr96 = load ptr, ptr %"$tmp", align 8
  call void @kai_release(ptr %rel.hdr96)
  call void @kai.release_Point(ptr %pt)
  %rel.hdr97 = load ptr, ptr %words, align 8
  call void @kai_release(ptr %rel.hdr97)
  %rel.hdr98 = load ptr, ptr %ns, align 8
  call void @kai_release(ptr %rel.hdr98)
  ret i32 %tmp95

panic92:                                          ; preds = %if.then87
  call void @kai_panic(ptr @kai.panic.msg.19, i64 16, ptr @kai.src.file, i64 25, i64 20)
  unreachable

arith.ok93:                                       ; preds = %if.then87
  %add94 = extractvalue { i32, i1 } %ovf90, 0
  store i32 %add94, ptr %total, align 4
  br label %if.end88
}

declare void @kai_reversible_enter()

declare void @kai_retain(ptr)

define void @kai.retain_Point(ptr %0) {
entry:
  %fld = getelementptr inbounds nuw %Point, ptr %0, i32 0, i32 1
  %fld.v = load ptr, ptr %fld, align 8
  call void @kai_retain(ptr %fld.v)
  ret void
}

declare void @kai_reversible_unwind()

declare void @kai_panic(ptr, i64, ptr, i64, i64)

declare void @kai_reversible_push(ptr, ptr, i64, ptr)

define void @kai.snapREL_string(ptr %0) {
entry:
  %rel.hdr = load ptr, ptr %0, align 8
  call void @kai_release(ptr %rel.hdr)
  ret void
}

declare void @kai_release(ptr)

declare ptr @kai_string_new(ptr, i64)

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.sadd.with.overflow.i32(i32, i32) #0

define void @kai.release_Point(ptr %0) {
entry:
  %fld = getelementptr inbounds nuw %Point, ptr %0, i32 0, i32 1
  %rel.hdr = load ptr, ptr %fld, align 8
  call void @kai_release(ptr %rel.hdr)
  ret void
}

declare void @kai_reversible_commit()

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

declare i8 @kai_string_eq(ptr, ptr)

attributes #0 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
