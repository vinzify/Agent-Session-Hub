function Compress-CshText {
    param(
        [string]$Text,
        [int]$MaxLength = 120
    )

    if ([string]::IsNullOrWhiteSpace($Text)) {
        return ''
    }

    $clean = ($Text -replace '\s+', ' ').Trim()
    if ($clean.Length -le $MaxLength) {
        return $clean
    }

    if ($MaxLength -le 3) {
        return '.' * $MaxLength
    }

    return '{0}...' -f $clean.Substring(0, $MaxLength - 3)
}

function Get-CshProjectName {
    param([string]$ProjectPath)

    if ([string]::IsNullOrWhiteSpace($ProjectPath)) {
        return '<unknown>'
    }

    $leaf = Split-Path -Leaf $ProjectPath
    if ([string]::IsNullOrWhiteSpace($leaf)) {
        return $ProjectPath
    }

    return $leaf
}

function Format-CshTimestamp {
    param([datetimeoffset]$Timestamp)

    return $Timestamp.ToLocalTime().ToString('yyyy-MM-dd HH:mm')
}

function Format-CshRelativeAge {
    param([datetimeoffset]$Timestamp)

    $now = [datetimeoffset]::Now
    $delta = $now - $Timestamp.ToLocalTime()

    if ($delta.TotalSeconds -lt 60) {
        $seconds = [Math]::Max(1, [int][Math]::Floor($delta.TotalSeconds))
        return '{0}s ago' -f $seconds
    }

    if ($delta.TotalMinutes -lt 60) {
        $minutes = [int][Math]::Floor($delta.TotalMinutes)
        return '{0}m ago' -f $minutes
    }

    if ($delta.TotalHours -lt 24) {
        $hours = [int][Math]::Floor($delta.TotalHours)
        return '{0}h ago' -f $hours
    }

    $days = [int][Math]::Floor($delta.TotalDays)
    return '{0}d ago' -f $days
}

function Format-CshAsciiBanner {
    param(
        [string]$Kind,
        [string]$Primary,
        [string]$Secondary
    )

    $kindText = if ([string]::IsNullOrWhiteSpace($Kind)) { 'item' } else { $Kind.ToUpperInvariant() }
    $primaryText = if ([string]::IsNullOrWhiteSpace($Primary)) { '-' } else { Compress-CshText -Text $Primary -MaxLength 36 }
    $secondaryText = if ([string]::IsNullOrWhiteSpace($Secondary)) { '' } else { Compress-CshText -Text $Secondary -MaxLength 18 }
    $headline = if ($secondaryText) { '{0} | {1} | {2}' -f $kindText, $primaryText, $secondaryText } else { '{0} | {1}' -f $kindText, $primaryText }
    $border = '+' + ('-' * $headline.Length) + '+'

    return (@(
        $border
        ('|{0}|' -f $headline)
        $border
    ) -join [Environment]::NewLine)
}
