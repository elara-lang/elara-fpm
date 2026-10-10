#!/usr/bin/env bash
# Resolve Elara C ABI paths (elara.h + libelara).
# Default is /usr/local when that prefix holds the install; otherwise a sibling checkout.
# Usage: source from other .make scripts, or: .make/elara-env.sh [--print|--check]
set -euo pipefail

elara_http_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

elara_fail() {
	echo "error: Elara runtime not found ($1)." >&2
	echo "  Install Elara under /usr/local (include/elara.h and lib/libelara.{a,so})," >&2
	echo "  set ELARA_PREFIX to another install prefix," >&2
	echo "  or set ELARA_ROOT to an Elara repository root (sibling checkout), or place Elara at ../elara." >&2
	echo "  If both ELARA_PREFIX and ELARA_ROOT are set, install mode (ELARA_PREFIX) wins." >&2
	echo "  With neither set, /usr/local is used when that install is present; otherwise ../elara." >&2
	return 1
}

# True when prefix holds the installed C ABI (header + static or shared library).
elara_install_present() {
	local prefix="$1"
	[[ -f "${prefix}/include/elara.h" ]] || return 1
	[[ -f "${prefix}/lib/libelara.so" || -f "${prefix}/lib/libelara.a" ]]
}

elara_resolve() {
	ELARA_MODE=""
	ELARA_INCLUDE=""
	ELARA_LIBDIR=""
	ELARA_LIB=""

	local prefix="${ELARA_PREFIX:-}"
	local root="${ELARA_ROOT:-}"
	local default_prefix="/usr/local"

	if [[ -n "$prefix" ]]; then
		if [[ ! -d "$prefix" ]]; then
			elara_fail "ELARA_PREFIX is not a directory: ${prefix}"
			return 1
		fi
		ELARA_MODE="install"
		ELARA_INCLUDE="${prefix}/include"
		ELARA_LIBDIR="${prefix}/lib"
	elif [[ -z "$root" ]] && elara_install_present "$default_prefix"; then
		# System install wins over a sibling checkout unless ELARA_ROOT is set.
		prefix="$default_prefix"
		ELARA_MODE="install"
		ELARA_INCLUDE="${prefix}/include"
		ELARA_LIBDIR="${prefix}/lib"
	else
		if [[ -z "$root" ]]; then
			local sibling="${elara_http_root}/../elara"
			if [[ -d "$sibling" ]]; then
				root="$(cd "$sibling" && pwd)"
			fi
		fi
		if [[ -n "$root" ]]; then
			if [[ ! -d "$root" ]]; then
				elara_fail "ELARA_ROOT is not a directory: ${root}"
				return 1
			fi
			root="$(cd "$root" && pwd)"
			ELARA_MODE="sibling"
			ELARA_INCLUDE="${root}/elara/include"
			if [[ -f "${root}/bin/libelara.a" || -f "${root}/bin/libelara.so" ]]; then
				ELARA_LIBDIR="${root}/bin"
			elif [[ -f "${root}/elara/target/release/libelara.a" || -f "${root}/elara/target/release/libelara.so" ]]; then
				ELARA_LIBDIR="${root}/elara/target/release"
			else
				ELARA_LIBDIR="${root}/bin"
			fi
		fi
	fi

	if [[ -z "$ELARA_MODE" ]]; then
		elara_fail "neither /usr/local, ELARA_PREFIX, ELARA_ROOT, nor ../elara"
		return 1
	fi

	if [[ ! -f "${ELARA_INCLUDE}/elara.h" ]]; then
		elara_fail "missing ${ELARA_INCLUDE}/elara.h"
		return 1
	fi

	if [[ -f "${ELARA_LIBDIR}/libelara.so" ]]; then
		ELARA_LIB="${ELARA_LIBDIR}/libelara.so"
	elif [[ -f "${ELARA_LIBDIR}/libelara.a" ]]; then
		ELARA_LIB="${ELARA_LIBDIR}/libelara.a"
	else
		elara_fail "missing libelara.a/libelara.so under ${ELARA_LIBDIR}"
		return 1
	fi

	export ELARA_MODE ELARA_INCLUDE ELARA_LIBDIR ELARA_LIB
	if [[ -n "${ELARA_ROOT:-}" ]]; then
		export ELARA_ROOT
	elif [[ "$ELARA_MODE" == "sibling" ]]; then
		export ELARA_ROOT="$(cd "${ELARA_INCLUDE}/../.." && pwd)"
	fi
	if [[ -n "${ELARA_PREFIX:-}" ]]; then
		export ELARA_PREFIX
	fi
}

elara_print() {
	elara_resolve
	echo "ELARA_MODE=${ELARA_MODE}"
	echo "ELARA_INCLUDE=${ELARA_INCLUDE}"
	echo "ELARA_LIBDIR=${ELARA_LIBDIR}"
	echo "ELARA_LIB=${ELARA_LIB}"
}

main() {
	case "${1:-}" in
	--print)
		elara_print
		;;
	--check | "")
		elara_resolve
		;;
	*)
		echo "usage: $0 [--print|--check]" >&2
		return 2
		;;
	esac
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
	main "$@"
fi
