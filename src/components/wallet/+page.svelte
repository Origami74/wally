<script lang="ts">

    let balance = $state(0)
    let unit = $state("SAT")
    let withDrawTokenValue = $state("")

    function deposit(){
        console.log("deposit")
    }

    function withDrawToken(){
        console.log("withDrawToken")
    }
</script>

<div class="grid md:grid-cols-3 gap-2 p-4">

    <div class="flex bg-card text-card-foreground rounded-xl border shadow col-span-1 gap-2 p-2">
        <h1 class="">{balance} {unit}</h1>
    </div>
    <div class="grid md:grid-cols-3 col-span-2">
        <div class="flex col-span-2 gap-2" >
            <input placeholder="cashuAeyJ0b2tlb..."/>
        </div>
        <div class="flex col-span-1 gap-2 pl-2">
            <button on:click={deposit}>Deposit</button>
        </div>
        <div class="flex col-span-2 gap-2 pt-2" >
            <input bind:value={withDrawTokenValue} placeholder="token will appear here..."/>
        </div>
        <div class="flex col-span-1 gap-2 pl-2 pt-2">
            <button on:click={withDrawToken}>Withdraw</button>
        </div>
    </div>
</div>