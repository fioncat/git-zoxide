_GIT_ZOXIDE_CMDS=( \
	"attach" \
	"clean" \
	"detach" \
	"home" \
	"list" \
	"remove" \
	"config" \
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
			_git-zoxide_cmp_remote
			_git-zoxide_cmp_repo
			;;
		list)
			_git-zoxide_cmp_remote
			;;
		remove)
			_git-zoxide_cmp_remote
			_git-zoxide_cmp_repo
			;;
	esac
	if (( ${#words[@]} > 4 )); then
		_arguments '*:dir:_dirs'
	fi
}

_git-zoxide_cmp_remote() {
	if [ "${#words[@]}" -eq "3" ]; then
		local remotes=($($cmd list))
		_describe 'command' remotes
		return
	fi
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
