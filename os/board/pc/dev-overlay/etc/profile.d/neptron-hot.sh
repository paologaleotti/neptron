# Development variant: the binaries of the hot folder come first on the
# PATH of a shell, as they do for the launch script.
if [ -d /state/hot/bin ] && [ ! -e /state/hot/DISABLED ]; then
	PATH=/state/hot/bin:$PATH
fi
