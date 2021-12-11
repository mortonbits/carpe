<script lang="ts">
  import { restoreProofFromChain } from "../../miner_invoke";
</script>

<main class="uk-margin" >
  <div class="uk-grid">
    <div class="margin">
      Seems like the local proof is lost. (File: proof_***.json)<br/>
      <button class="uk-button uk-button-default" on:click={restoreProofFromChain}>Fetch latest proof from chain</button>
    </div>
  </div>
</main>
