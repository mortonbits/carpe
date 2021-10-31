import { invoke } from '@tauri-apps/api/tauri';
import { get, writable } from 'svelte/store';
import { raise_error } from './carpeError';
import { responses } from './debug';
import { getCurrent } from '@tauri-apps/api/window'

export interface VDFProof {
  height: number,
  elapsed_secs: number,
  preimage: string,
  proof: string,
  difficulty: number,
  security: number
}

export interface TowerStateView {
  previous_proof_hash: string,
  verified_tower_height: number, // user's latest verified_tower_height
  latest_epoch_mining: number,
  count_proofs_in_epoch: number,
  epochs_validating_and_mining: number,
  contiguous_epochs_validating_and_mining: number,
  epochs_since_last_account_creation: number
}

export interface ClientTowerStatus {
  latest_proof: VDFProof,
  on_chain: TowerStateView,
  count_proofs_this_session: number,
}

export const tower = writable<ClientTowerStatus>({});


export const towerOnce = async () => {
  console.log("mine tower once")

  let previous_duration = 30 * 60 * 1000;
  let t = get(tower);
  if (t.latest_proof && t.latest_proof.elapsed_secs) {
    previous_duration = t.latest_proof.elapsed_secs * 1000
  }
  
  let progress: ProofProgress =  {
    time_start: Date.now(),
    previous_duration,
  }
  proofState.set(progress);

  const current = getCurrent();
  current.emit('tower-make-proof', 'Tauri is awesome!');

};

export function startTowerListener() {
    invoke("build_tower", {})
    .then((res) => {
      console.log("tower response");
      console.log(res);
      responses.set(res);
    })
    .catch((e) => raise_error(e));
}

function incrementMinerStatus(new_proof: VDFProof): ClientTowerStatus {
  let m = get(tower);
  m.latest_proof = new_proof;
  m.count_proofs_this_session = m.count_proofs_this_session + 1;
  tower.set(m);
  return m;
}

function refreshOnChainData(on_chain: TowerStateView): ClientTowerStatus {
  let m = get(tower);
  m.on_chain = on_chain;
  tower.set(m);
  return m;
}


export const getTowerChainView = async () => {
  invoke("get_onchain_tower_state", {})
    .then((res: TowerStateView) => {
      console.log(res);
      refreshOnChainData(res);
      responses.set(res);
      
    })
    .catch((e) => raise_error(e));
};


export const miner_loop_enabled = writable(false);

export function toggleMining() {
  let enabled = get(miner_loop_enabled)
  if (enabled) {
    miner_loop_enabled.set(false);
  } else if (!enabled) {
    miner_loop_enabled.set(true);

    // careful to not start the miner twice.
    // the miner may be turned off, but a proof may still be running in the background.
    if (!isInProgress()) { 
      
      // start miner
      towerOnce()
    }
  };
  console.log(get(miner_loop_enabled));
}

function isInProgress():boolean {
  let ps = get(proofState);
  if (
    ps.time_start && 
    ps.time_start > 0 &&
    !ps.complete &&
    !ps.error
   ) {
    return true
  }
  return false;
}

export function proofError() {
  let ps = get(proofState);
  ps.error = true;
  proofState.set(ps);
}

export function proofComplete() {
  let ps = get(proofState);
  ps.complete = true;
  proofState.set(ps);
}
export interface ProofProgress {
  time_start: number,
  previous_duration: number,
  complete: boolean,
  error: boolean
}

export const proofState = writable<ProofProgress>({});
