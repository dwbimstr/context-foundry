using namespace System.Collections.Generic
using module ./Helpers.psm1
Import-Module Az.Accounts
. ./lib/common.ps1

function Get-Thing {
    [CmdletBinding()]
    param([string]$Name)
    $local = 1
    function Inner-Helper { 1 }
}

class Store {
    [int]$Count

    Store() {}

    [void] Put([int]$a) {}
    [void] Put([string]$a) {}
}

enum Color {
    Red
    Green = 2
}

$Script:Limit = 10
