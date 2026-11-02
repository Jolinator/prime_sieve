#!/usr/bin/env bash

# Collect the Linux host details this program uses for tuning (cores, cache sizes, memory).
# Usage: bash collect_linux.sh [output.env]
set -u

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
output_file="${1:-${script_dir}/.env}"
output_dir="$(dirname -- "$output_file")"
mkdir -p -- "$output_dir" || exit 1
tmp_file="${output_file}.tmp.$$"
trap 'rm -f -- "$tmp_file"' EXIT
: > "$tmp_file" || exit 1

env_put() {
    local key="$1"
    local value="${2:-}"
    [[ -n "$value" ]] || return 0
    value=${value//\\/\\\\}
    value=${value//\"/\\\"}
    value=${value//$'\r'/\\r}
    value=${value//$'\n'/\\n}
    printf '%s="%s"\n' "$key" "$value" >> "$tmp_file"
}

first_cpu_value() {
    local key="$1"
    awk -F: -v key="$key" '$1 ~ "^[[:space:]]*" key "[[:space:]]*$" { sub(/^[[:space:]]+/, "", $2); print $2; exit }' /proc/cpuinfo 2>/dev/null
}

read_value() {
    local path="$1"
    if [[ -r "$path" ]]; then
        tr -d '\n' < "$path"
    fi
}

logical_cores="$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '%s' "$(grep -c '^processor[[:space:]]*:' /proc/cpuinfo 2>/dev/null || true)")"
logical_cores="${logical_cores//[[:space:]]/}"
physical_cores="$(
    shopt -s nullglob
    for cpu_dir in /sys/devices/system/cpu/cpu[0-9]*; do
        [[ -d "$cpu_dir/topology" ]] || continue
        if [[ -r "$cpu_dir/online" ]] && [[ "$(read_value "$cpu_dir/online")" == "0" ]]; then
            continue
        fi
        package="$(read_value "$cpu_dir/topology/physical_package_id")"
        core="$(read_value "$cpu_dir/topology/core_id")"
        [[ -n "$package" && -n "$core" ]] && printf '%s:%s\n' "$package" "$core"
    done | sort -u | wc -l | tr -d '[:space:]'
)"

l1d_size=""
l2_size=""
l3_size=""
cache_line_sizes=""
shopt -s nullglob
for cache_dir in /sys/devices/system/cpu/cpu[0-9]*/cache/index*; do
    [[ -d "$cache_dir" ]] || continue
    level="$(read_value "$cache_dir/level")"
    type="$(read_value "$cache_dir/type")"
    size="$(read_value "$cache_dir/size")"
    size_kb="$(awk -v value="$size" 'BEGIN { unit=substr(value,length(value)); amount=substr(value,1,length(value)-1)+0; if (unit=="K") printf "%.0f", amount; else if (unit=="M") printf "%.0f", amount*1024; else if (unit=="G") printf "%.0f", amount*1048576; else printf "%.0f", value/1024 }')"
    line_size="$(read_value "$cache_dir/coherency_line_size")"
    [[ -n "$level" && -n "$type" && -n "$size" ]] || continue

    [[ -n "$line_size" ]] && cache_line_sizes+="${line_size}"$'\n'
    case "${level}:${type}" in
        1:Data) l1d_size+="${size_kb}"$'\n' ;;
        2:*) l2_size+="${size_kb}"$'\n' ;;
        3:*) l3_size+="${size_kb}"$'\n' ;;
    esac
done
cache_line_sizes="$(printf '%s' "$cache_line_sizes" | sort -nu | paste -sd, -)"
l1d_size="$(printf '%s' "$l1d_size" | sort -nu | paste -sd, -)"
l2_size="$(printf '%s' "$l2_size" | sort -nu | paste -sd, -)"
l3_size="$(printf '%s' "$l3_size" | sort -nu | paste -sd, -)"

mem_total_kb="$(awk '/^MemTotal:/ {print $2; exit}' /proc/meminfo 2>/dev/null)"

# Cache sizes are comma separated when a hybrid CPU has different core types.
env_put CPU_MODEL "$(first_cpu_value 'model name')"
env_put CPU_PHYSICAL_CORES "$physical_cores"
env_put CPU_LOGICAL_CORES "$logical_cores"
env_put CPU_CACHE_LINE_SIZE_BYTES "$cache_line_sizes"
env_put CPU_L1D_CACHE_SIZE_PER_INSTANCE_KB "$l1d_size"
env_put CPU_L2_CACHE_SIZE_PER_INSTANCE_KB "$l2_size"
env_put CPU_L3_CACHE_SIZE_PER_INSTANCE_KB "$l3_size"
env_put PHYSICAL_MEMORY_TOTAL_KB "$mem_total_kb"

mv -f -- "$tmp_file" "$output_file" || exit 1
trap - EXIT
printf 'Wrote system details to %s\n' "$output_file"
