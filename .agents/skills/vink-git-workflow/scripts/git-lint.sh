#!/bin/sh
# Kiểm quy ước Git của Vink (xem ../SKILL.md): tên nhánh, commit message, tiêu đề PR.
#
#   git-lint.sh branch [NAME]     tên nhánh (mặc định: nhánh hiện tại)
#   git-lint.sh message FILE      commit message (dùng cho hook commit-msg)
#   git-lint.sh title TEXT        tiêu đề PR
#   git-lint.sh pre-push          đọc stdin của hook pre-push; chỉ kiểm nhánh remote mới
#
# Thoát 0 khi đạt (có thể kèm cảnh báo), 1 khi vi phạm, 2 khi dùng sai.
set -u
set -f

TYPES='feat|fix|refactor|perf|chore|docs|test|revert'
BRANCH_TYPES='feat|fix|hotfix|refactor|perf|chore|docs|test'
TOOL_PREFIXES='codex|claude|ai|agent|agents|copilot|cursor|gemini|antigravity|devin'
INTEGRATION='develop|main|master'
TRAILER_KEYS='refs|decision|spec|design|evidence|breaking change|breaking-change|co-authored-by|signed-off-by'
MAX_BRANCH=50
MAX_SUBJECT=72
MAX_TITLE=64

status=0
fail() { printf 'git-lint: %s\n' "$*" >&2; status=1; }
warn() { printf 'git-lint: cảnh báo: %s\n' "$*" >&2; }
list() { printf '%s' "$1" | sed 's/|/, /g'; }

# Độ dài theo ký tự Unicode; thiếu perl thì đếm byte (chặt hơn).
chars() {
	if command -v perl >/dev/null 2>&1; then
		printf '%s' "$1" | perl -CSD -e 'local $/; my $s = <STDIN>; print length($s // "")'
	else
		printf '%s' "$1" | wc -c | tr -d ' '
	fi
}

usage() {
	sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'
}

check_branch() {
	name=$1
	if [ -z "$name" ]; then
		fail 'không xác định được nhánh (detached HEAD?): tạo nhánh <type>/<issue>-<slug> trước khi commit'
		return
	fi
	if printf '%s\n' "$name" | grep -Eq "^($INTEGRATION)\$"; then
		fail "'$name' là nhánh tích hợp: tạo nhánh <type>/<issue>-<slug> từ origin/develop, trừ khi người dùng yêu cầu commit thẳng"
		return
	fi
	if printf '%s\n' "$name" | grep -Eiq "^($TOOL_PREFIXES)/"; then
		fail "'$name' mang tiền tố công cụ: đổi bằng 'git branch -m' khi nhánh chưa có trên remote và chưa có PR"
		return
	fi
	if ! printf '%s\n' "$name" | grep -Eq "^($BRANCH_TYPES)/([0-9]+-)?[a-z0-9]+(-[a-z0-9]+)*\$"; then
		fail "'$name' không theo <type>/<issue>-<slug>, type thuộc: $(list "$BRANCH_TYPES"); chỉ a-z, 0-9 và '-'"
	fi
	n=$(chars "$name")
	[ "${n:-0}" -le "$MAX_BRANCH" ] || fail "'$name' dài $n ký tự (tối đa $MAX_BRANCH)"
}

is_git_generated() {
	case $1 in
	'Merge '* | 'Revert "'* | 'fixup! '* | 'squash! '* | 'amend! '*) return 0 ;;
	esac
	return 1
}

check_subject() {
	s=$1
	max=$2
	what=$3
	is_git_generated "$s" && return
	if ! printf '%s\n' "$s" | grep -Eq "^($TYPES)(\([a-z0-9][a-z0-9._/-]*\))?!?: [^ ]"; then
		fail "$what phải có dạng 'type(scope): mô tả', type thuộc: $(list "$TYPES"); nhận: '$s'"
	fi
	n=$(chars "$s")
	[ "${n:-0}" -le "$max" ] || fail "$what dài $n ký tự (tối đa $max)"
	case $s in
	*.) fail "$what không kết thúc bằng dấu chấm" ;;
	esac
	if printf '%s\n' "$s" | grep -Eq '\(#[0-9]+(, ?#[0-9]+)*\)$'; then
		warn "$what không cần '(#N)': issue nằm ở trailer hoặc mục Liên kết, GitHub tự nối số PR"
	fi
}

check_message() {
	file=$1
	if [ ! -r "$file" ]; then
		fail "không đọc được '$file'"
		return
	fi
	# Bỏ dòng comment của git và phần sau dòng cắt của 'git commit -v'.
	msg=$(sed -e '/^# -\{24\} >8 -\{24\}$/,$d' -e '/^#/d' "$file" | sed '/./,$!d')
	if [ -z "$msg" ]; then
		fail 'commit message rỗng'
		return
	fi
	subject=$(printf '%s\n' "$msg" | sed -n 1p)
	check_subject "$subject" "$MAX_SUBJECT" 'subject commit'
	is_git_generated "$subject" && return

	second=$(printf '%s\n' "$msg" | sed -n 2p)
	[ -z "$second" ] || fail 'dòng thứ hai phải trống: subject, một dòng trống, rồi body'

	if printf '%s\n' "$msg" | grep -Eiq '#TBD|https?://[^ ]*(\.\.\.|…)'; then
		fail 'có placeholder hoặc URL rút gọn: chỉ ghi nguồn có thật, không có thì bỏ trailer'
	fi
	if printf '%s\n' "$msg" | grep -Eq '\[[^]]+\]\(https?://'; then
		fail 'commit là plain text: dùng trailer "Key: URL" thay cho [nhãn](URL)'
	fi
	if printf '%s\n' "$msg" | grep -Eiq '^(close[sd]?|fix(e[sd])?|resolve[sd]?):? +([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+)?#[0-9]+'; then
		warn 'keyword đóng issue trong commit sẽ đóng issue khi commit tới develop; dùng "Refs:" và để PR body quyết định "Closes"'
	fi
	if printf '%s\n' "$msg" | grep -Eq '^Refs #[0-9]+'; then
		warn 'dùng dạng trailer "Refs: #N" (có dấu hai chấm) để git đọc được'
	fi

	# Đoạn cuối gồm toàn dòng "Key: value" là trailer; kiểm key và giá trị.
	paragraphs=$(printf '%s\n' "$msg" | awk 'BEGIN { RS = "" } END { print NR }')
	[ "${paragraphs:-1}" -gt 1 ] || return
	problems=$(printf '%s\n' "$msg" | awk -v keys="$TRAILER_KEYS" '
		BEGIN { RS = ""; FS = "\n"; n = split(keys, k, "|"); for (i = 1; i <= n; i++) ok[k[i]] = 1 }
		{ last = $0 }
		END {
			m = split(last, line, "\n")
			for (i = 1; i <= m; i++)
				if (line[i] !~ /^[A-Za-z][A-Za-z-]*( [A-Za-z-]+)?: /) exit
			for (i = 1; i <= m; i++) {
				p = index(line[i], ": ")
				key = tolower(substr(line[i], 1, p - 1))
				val = substr(line[i], p + 2)
				gsub(/^[ \t]+|[ \t]+$/, "", val)
				if (key ~ /^(close[sd]?|fix(e[sd])?|resolve[sd]?)$/) continue
				if (!(key in ok)) print "WARN:trailer lạ \"" substr(line[i], 1, p - 1) "\": dùng Refs, Decision, Spec, Design, Evidence, BREAKING CHANGE"
				if (val == "" || toupper(val) ~ /^(TBD|TODO|N\/A|NONE|\?+|\.\.\.)$/ || tolower(val) ~ /<(link|url|issue|sha|id)>/)
					print "FAIL:trailer \"" substr(line[i], 1, p - 1) "\" thiếu giá trị thật; không có nguồn thì bỏ dòng"
			}
		}')
	old_ifs=$IFS
	IFS='
'
	for p in $problems; do
		case $p in
		FAIL:*) fail "${p#FAIL:}" ;;
		WARN:*) warn "${p#WARN:}" ;;
		esac
	done
	IFS=$old_ifs
}

pre_push() {
	while read -r local_ref local_sha remote_ref remote_sha; do
		[ -n "${remote_sha:-}" ] || continue
		case $remote_ref in refs/heads/*) ;; *) continue ;; esac
		case $local_sha in *[!0]*) ;; *) continue ;; esac   # xoá nhánh remote
		case $remote_sha in *[!0]*) continue ;; esac        # nhánh đã có: giữ tên cũ
		name=${remote_ref#refs/heads/}
		printf '%s\n' "$name" | grep -Eq "^($INTEGRATION)\$" && continue # khởi tạo remote
		check_branch "$name"
	done
	[ "$status" -eq 0 ] || printf 'git-lint: đổi tên nhánh rồi push lại (git branch -m <tên-mới>)\n' >&2
}

cmd=${1:-}
[ $# -gt 0 ] && shift
case $cmd in
branch) check_branch "${1:-$(git branch --show-current 2>/dev/null)}" ;;
message | msg)
	if [ $# -lt 1 ]; then
		usage >&2
		exit 2
	fi
	check_message "$1"
	;;
title) check_subject "${1:-}" "$MAX_TITLE" 'tiêu đề PR' ;;
pre-push) pre_push ;;
-h | --help)
	usage
	exit 0
	;;
*)
	usage >&2
	exit 2
	;;
esac
exit "$status"
