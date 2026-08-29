; ModuleID = 'kai_module'
source_filename = "kai_module"

%domain.user.User.1 = type { i32, i1 }
%domain.user.User = type { i32, i1 }
%domain.user.User.0 = type { i32, i1 }

define i32 @main() {
entry:
  %u = alloca %domain.user.User.1, align 8
  %call = call %domain.user.User.1 @services.auth.login(i32 42)
  store %domain.user.User.1 %call, ptr %u, align 4
  %field = getelementptr inbounds nuw %domain.user.User.1, ptr %u, i32 0, i32 0
  %field1 = load i32, ptr %field, align 4
  %eq = icmp eq i32 %field1, 42
  br i1 %eq, label %if.then, label %if.end

if.then:                                          ; preds = %entry
  %field2 = getelementptr inbounds nuw %domain.user.User.1, ptr %u, i32 0, i32 1
  %field3 = load i1, ptr %field2, align 1
  %not = xor i1 %field3, true
  br i1 %not, label %if.then4, label %if.end5

if.end:                                           ; preds = %if.end5, %entry
  ret i32 1

if.then4:                                         ; preds = %if.then
  ret i32 0

if.end5:                                          ; preds = %if.then
  br label %if.end
}

define %domain.user.User.1 @services.auth.login(i32 %id) {
entry:
  %inactive = alloca %domain.user.User.1, align 8
  %user = alloca %domain.user.User.1, align 8
  %id1 = alloca i32, align 4
  store i32 %id, ptr %id1, align 4
  %tmp = load i32, ptr %id1, align 4
  %call = call %domain.user.User.1 @domain.user.User.create.3(i32 %tmp)
  store %domain.user.User.1 %call, ptr %user, align 4
  %tmp2 = load %domain.user.User.1, ptr %user, align 4
  %call3 = call %domain.user.User.1 @domain.user.User.deactivate.4(%domain.user.User.1 %tmp2)
  store %domain.user.User.1 %call3, ptr %inactive, align 4
  %tmp4 = load %domain.user.User.1, ptr %inactive, align 4
  ret %domain.user.User.1 %tmp4
}

define %domain.user.User @domain.user.User.create(i32 %id) {
entry:
  %tmp = alloca %domain.user.User, align 8
  %id1 = alloca i32, align 4
  store i32 %id, ptr %id1, align 4
  %tmp2 = load i32, ptr %id1, align 4
  %f = getelementptr inbounds nuw %domain.user.User, ptr %tmp, i32 0, i32 0
  store i32 %tmp2, ptr %f, align 4
  %f3 = getelementptr inbounds nuw %domain.user.User, ptr %tmp, i32 0, i32 1
  store i1 true, ptr %f3, align 1
  %lit = load %domain.user.User, ptr %tmp, align 4
  ret %domain.user.User %lit
}

define %domain.user.User @domain.user.User.deactivate(%domain.user.User %u) {
entry:
  %tmp = alloca %domain.user.User, align 8
  %u1 = alloca %domain.user.User, align 8
  store %domain.user.User %u, ptr %u1, align 4
  %field = getelementptr inbounds nuw %domain.user.User, ptr %u1, i32 0, i32 0
  %field2 = load i32, ptr %field, align 4
  %f = getelementptr inbounds nuw %domain.user.User, ptr %tmp, i32 0, i32 0
  store i32 %field2, ptr %f, align 4
  %f3 = getelementptr inbounds nuw %domain.user.User, ptr %tmp, i32 0, i32 1
  store i1 false, ptr %f3, align 1
  %lit = load %domain.user.User, ptr %tmp, align 4
  ret %domain.user.User %lit
}

define %domain.user.User.0 @domain.user.User.create.1(i32 %id) {
entry:
  %tmp = alloca %domain.user.User.0, align 8
  %id1 = alloca i32, align 4
  store i32 %id, ptr %id1, align 4
  %tmp2 = load i32, ptr %id1, align 4
  %f = getelementptr inbounds nuw %domain.user.User.0, ptr %tmp, i32 0, i32 0
  store i32 %tmp2, ptr %f, align 4
  %f3 = getelementptr inbounds nuw %domain.user.User.0, ptr %tmp, i32 0, i32 1
  store i1 true, ptr %f3, align 1
  %lit = load %domain.user.User.0, ptr %tmp, align 4
  ret %domain.user.User.0 %lit
}

define %domain.user.User.0 @domain.user.User.deactivate.2(%domain.user.User.0 %u) {
entry:
  %tmp = alloca %domain.user.User.0, align 8
  %u1 = alloca %domain.user.User.0, align 8
  store %domain.user.User.0 %u, ptr %u1, align 4
  %field = getelementptr inbounds nuw %domain.user.User.0, ptr %u1, i32 0, i32 0
  %field2 = load i32, ptr %field, align 4
  %f = getelementptr inbounds nuw %domain.user.User.0, ptr %tmp, i32 0, i32 0
  store i32 %field2, ptr %f, align 4
  %f3 = getelementptr inbounds nuw %domain.user.User.0, ptr %tmp, i32 0, i32 1
  store i1 false, ptr %f3, align 1
  %lit = load %domain.user.User.0, ptr %tmp, align 4
  ret %domain.user.User.0 %lit
}

define %domain.user.User.1 @domain.user.User.create.3(i32 %id) {
entry:
  %tmp = alloca %domain.user.User.1, align 8
  %id1 = alloca i32, align 4
  store i32 %id, ptr %id1, align 4
  %tmp2 = load i32, ptr %id1, align 4
  %f = getelementptr inbounds nuw %domain.user.User.1, ptr %tmp, i32 0, i32 0
  store i32 %tmp2, ptr %f, align 4
  %f3 = getelementptr inbounds nuw %domain.user.User.1, ptr %tmp, i32 0, i32 1
  store i1 true, ptr %f3, align 1
  %lit = load %domain.user.User.1, ptr %tmp, align 4
  ret %domain.user.User.1 %lit
}

define %domain.user.User.1 @domain.user.User.deactivate.4(%domain.user.User.1 %u) {
entry:
  %tmp = alloca %domain.user.User.1, align 8
  %u1 = alloca %domain.user.User.1, align 8
  store %domain.user.User.1 %u, ptr %u1, align 4
  %field = getelementptr inbounds nuw %domain.user.User.1, ptr %u1, i32 0, i32 0
  %field2 = load i32, ptr %field, align 4
  %f = getelementptr inbounds nuw %domain.user.User.1, ptr %tmp, i32 0, i32 0
  store i32 %field2, ptr %f, align 4
  %f3 = getelementptr inbounds nuw %domain.user.User.1, ptr %tmp, i32 0, i32 1
  store i1 false, ptr %f3, align 1
  %lit = load %domain.user.User.1, ptr %tmp, align 4
  ret %domain.user.User.1 %lit
}
