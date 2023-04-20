_GIT_ZOXIDE_CMDS=( \
	"attach" \
	"clean" \
	"detach" \
	"home" \
	"list" \
	"remove" \
	"config" \
	"open" \
	"merge" \
	"branch" \
	"rebase" \
	"reset" \
	"squash" \
	"jump" \
	"tag" \
)

_git-zoxide() {
	if [ "${#words[@]}" -eq "2" ]; then
		_alternative "arguments::($_GIT_ZOXIDE_CMDS)"
		return
	fi

	local cmd=${words[1]}
	local action=${words[2]}
	case $action in
		attach)
			_git-zoxide_cmp_remote
			_git-zoxide_cmp_group
			;;
		clean)
			;;
		detach)
			;;
		home)
			_git-zoxide_cmp_remote_keyword
			_git-zoxide_cmp_repo
			;;
		jump)
			_git-zoxide_cmp_keyword
			;;
		list)
			_git-zoxide_cmp_remote
			;;
		remove)
			_git-zoxide_cmp_remote
			_git-zoxide_cmp_repo
			;;
		branch)
			_git-zoxide_cmp_branch
			;;
		rebase)
			_git-zoxide_cmp_branch
			;;
		reset)
			_git-zoxide_cmp_branch
			;;
		squash)
			_git-zoxide_cmp_branch
			;;
		tag)
			_git-zoxide_cmp_tag
			;;
	esac
	if (( ${#words[@]} > 4 )); then
		_arguments '*:dir:_dirs'
	fi
}

_git-zoxide_cmp_remote() {
	if [ "${#words[@]}" -eq "3" ]; then
		local remotes=($($cmd list --remote 2>/dev/null))
		_describe 'command' remotes
		return
	fi
}

_git-zoxide_cmp_keyword() {
	if [ "${#words[@]}" -eq "3" ]; then
		local remotes=($($cmd list --keyword 2>/dev/null))
		_describe 'command' remotes
		return
	fi
}

_git-zoxide_cmp_remote_keyword() {
	if [ "${#words[@]}" -eq "3" ]; then
		local remotes=($($cmd list --remote --keyword 2>/dev/null))
		_describe 'command' remotes
		return
	fi
}

_git-zoxide_cmp_branch() {
	if [ "${#words[@]}" -eq "3" ]; then
		local branches=($($cmd branch --cmp 2>/dev/null))
		_describe 'command' branches
		return
	fi
}

_git-zoxide_cmp_tag() {
	if [ "${#words[@]}" -eq "3" ]; then
		local tags=($($cmd tag 2>/dev/null))
		_describe 'command' tags
		return
	fi
	local rules=($($cmd tag --show-rules 2>/dev/null))
	_describe 'command' rules
}

_git-zoxide_cmp_repo() {
	if [ "${#words[@]}" -eq "4" ]; then
		local remote=${words[3]}
		local repos=($($cmd list ${remote} 2>/dev/null))
		_describe 'command' repos
		return
	fi
}

_git-zoxide_cmp_group() {
	if [ "${#words[@]}" -eq "4" ]; then
		local remote=${words[3]}
		local groups=($($cmd list ${remote} --group 2>/dev/null))
		_describe 'command' groups -S ''
		return
	fi
}

compdef _git-zoxide git-zoxide
