_git-zoxide_home() {
	if ret_path=$(git-zoxide $@); then
		if [ -d $ret_path ]; then
			cd $ret_path
			return
		fi
		if [ ! -z $ret_path ]; then
			echo $ret_path
		fi
		return
	fi
	return 1
}

{{CMD}}() {
	action=$1
	case "${action}" in
		home)
			_git-zoxide_home $@
			;;

		*)
			git-zoxide $@
			;;
	esac
	return $?
}

compdef _git-zoxide {{CMD}}
alias {{HOME_CMD}}='{{CMD}} home'
