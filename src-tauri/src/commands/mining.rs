use std::{env, path::PathBuf};
use tokio::task;
use crate::{carpe_error::CarpeError, configs::{get_cfg, get_diem_client, get_tx_params}, configs_profile::get_local_proofs_this_profile};
use anyhow::Error;
use diem_json_rpc_types::views::{TowerStateResourceView, TransactionDataView};
use ol::config::AppCfg;
use ol_types::block::VDFProof;
use tauri::Window;
use tauri::Manager;
use tower::{backlog::process_backlog, commit_proof::{self, commit_proof_tx}, proof::mine_once};
use txs::submit_tx::{eval_tx_status, TxParams};
// use crate::configs::{get_cfg, get_tx_params};

/// A new listener needs to be started whenever the user changes profiles i.e. using a different signing account.
/// This is because the private key gets loaded in member when then listener is initialized.

//TODO: there's a risk of multiple tower listeners being initialized. This is handled on the JS window side, but we likely need more guarantees on the rust side. Unsure how to do this without implementing a proper queue.
#[tauri::command]
pub async fn start_tower_listener(window: Window) -> Result<(), CarpeError> {
  
  println!("starting tower builder, listening for tower-make-proof");
  // prepare listener to receive events
  // TODO: this is gross. Prevent cloning when using in closures
  let window_clone = window.clone();
  let new_clone = window_clone.clone();
  let config = get_cfg()?;

  // This is tauri's event listener for the tower proof.
  // the front-ent/window will keep calling it when it needs a new proof done.
  let h = window.listen("tower-make-proof", move |e| {
    println!("received tower-make-proof event");
    println!("received event {:?}", e);

    let third_clone = window_clone.clone();
    let config_clone = config.clone();

    // The VDF by definition will block the thread. The work needs to be sent to a thread that can be blocked.
    let _ = task::spawn_blocking( move || {
        // TODO: how to cehck for this before it get here?
        // tx params cannot be cloned.
        let tx_params = get_tx_params(None).expect("could not load tx params, this should have been checked before");
        // some blocking work here
        match mine_and_commit_one_proof(&config_clone, &tx_params) {
        Ok(proof) => {
          third_clone.emit("tower-event", proof).unwrap();
        }
        Err(e) => {
          third_clone.emit("tower-error", e).unwrap();
        }
      }
    });
  });

  window.once("kill-listener", move |_| {
    println!("received kill listener event");
    new_clone.unlisten(h);
  });

  Ok(())
}


#[derive(Clone, serde::Serialize)]
struct BacklogSuccess {
  success: bool,
}

#[tauri::command]
pub async fn submit_backlog(window: Window) -> Result<(), CarpeError> {
  let config = get_cfg()?;
  let tx_params = get_tx_params(None)
    .map_err(|_e| CarpeError::tower("could getch tx_params while sending backlog."))?;

  let _ = match backlog(&config, &tx_params) {
      Ok(_) => window.emit("backlog-success", BacklogSuccess {success: true}),
      Err(_) => window.emit("backlog-error", CarpeError::tower("could not submit backlog)"))
  };
    
  Ok(())
}


/// flush a backlog of proofs at once to the chain.
pub fn backlog(
  config: &AppCfg,
  tx_params: &TxParams,
) -> Result<(), CarpeError> {
  // TODO: This does not return an error on transaction failure. Change in upstream.
  process_backlog(config, tx_params, false) 
    .map_err(|e| { 
      CarpeError::tower(&format!("could not complete sending of backlog, message: {:?}", &e))
    })?;
  Ok(())
}

fn get_proof_zero() -> Result<VDFProof, Error> {
  let cfg = get_cfg()?;
  let path = cfg.workspace.node_home.join(cfg.workspace.block_dir).join("proof_0.json");
  let string = std::fs::read_to_string(path)?;
  let proof: VDFProof = serde_json::from_str(&string)?;
  dbg!(&proof);
  // .parse();
  Ok(proof)
}

#[tauri::command]
pub fn debug_submit_proof_zero() -> Result<(), CarpeError>{
  let tx_params = get_tx_params(None)
    .map_err(|_e| CarpeError::tower("could getch tx_params while sending backlog."))?;
  let proof = get_proof_zero()?;
  commit_proof_tx(&tx_params,proof, false)?;
  Ok(())
}


#[test]
fn test_proof_zero() {
  dbg!(&get_proof_zero());
}

/// creates one proof and submits
pub fn mine_and_commit_one_proof(
  config: &AppCfg,
  tx_params: &TxParams,
) -> Result<VDFProof, CarpeError> {
  println!("Mining one proof");
  match mine_once(&config) {
    Ok(b) => match commit_proof::commit_proof_tx(&tx_params, b.clone(), false) {
      Ok(tx_view) => match eval_tx_status(&tx_view) {
        Ok(_) => Ok(b),
        Err(e) => {
          let msg = format!(
            "ERROR: Tower proof NOT committed to chain, message: \n{:?}",
            e
          );
          println!("{}", &msg);
          Err(CarpeError::tower(&msg))
        }
      },
      Err(e) => {
        let msg = format!("Tower transaction rejected, message: \n{:?}", e);
        println!("{}", &msg);
        Err(CarpeError::tower(&msg))
      }
    },
    Err(e) => {
      let msg = format!("Error mining tower proof, message: {:?}", e);
      println!("{}", &msg);
      Err(CarpeError::tower(&msg))
    }
  }
}

// TODO: Resubmit backlog

#[tauri::command]
pub fn get_onchain_tower_state() -> Result<TowerStateResourceView, CarpeError> {
  println!("fetching onchain tower state");
  let cfg = get_cfg()?;
  let client = get_diem_client(&cfg)?;

  match client.get_miner_state(&cfg.profile.account) {
    Ok(Some(t)) => {
      dbg!(&t);
      Ok(t)
    }
    _ => Err(CarpeError::tower("could not get tower state from chain")),
  }
}


#[tauri::command]
pub fn get_local_proofs() -> Result<Vec<PathBuf>, CarpeError> {

  get_local_proofs_this_profile()
  // TODO: Why is the CarpeError From anyhow not working?
  .map_err(|e| { CarpeError::misc(&format!("could not get local files, message: {:?}", e.to_string()) ) })
}

#[tauri::command]
pub fn restore_proof_from_chain() -> Result<TowerStateResourceView, CarpeError> {
  println!("fetching latest proof from chain");
  let cfg = get_cfg()?;
  let mut client = get_diem_client(&cfg)?;

  let t = match client.get_miner_state(&cfg.profile.account) {
    Ok(Some(t)) => {
      t
    }
    _ => return Err(CarpeError::tower("could not get tower state from chain")),
  };

  println!("TowerStateResourceView = {:?}", t);

  let transactions = match client.get_txn_by_acc_range(cfg.profile.account, t.verified_tower_height, 1000,false) {
    Ok(transactions) => {
      transactions
    }
    Err(e) => {
      println!("get_txn_by_acc_range error: {:?}", e);
      return Err(CarpeError::tower("could not get transactions from chain. Please configure an upstream URL with full history."));
    }
  };

  println!("TransactionView = {:?}", transactions);
  println!("======================================================================");
  println!("TransactionView.len() = {:?}", transactions.len());
  println!("======================================================================");
  println!("transactions[0] = {:?}", transactions[0]);
  println!("======================================================================");
  println!("transactions[0].vm_status.is_executed() = {:?}", transactions[0].vm_status.is_executed());
  println!("======================================================================");
  println!("transactions[0].transaction = {:?}", transactions[0].transaction);
  println!("======================================================================");
  println!("transactions[0].bytes = {:?}", transactions[0].bytes);
  println!("======================================================================");

 /*
  println!("transactions[0].transaction.user = {:?}", transactions[0].transaction.user);
  println!("======================================================================");
  println!("transactions[0].transaction.user.script = {:?}", transactions[0].transaction.user.script);
  println!("======================================================================");
  println!("transactions[0].transaction.user.script.function_name = {:?}", transactions[0].transaction.user.script.function_name);
  println!("======================================================================");
  println!("transactions[0].transaction.user.script.function_name = {:?}", transactions[0].transaction.user.script.function_name);
  println!("======================================================================");
 */



  if transactions.len() > 0 && transactions.len() < 1000 {
    for i in (0..transactions.len()).rev() {
      println!("{:?} type - {:?}", i, transactions[i].vm_status.is_executed());

      // let userTransaction = TransactionDataView.UserTransaction::from(transactions[i].transaction);
      // let userTransaction :TransactionDataView::UserTransaction = transactions[i].transaction;
      // let userTransaction = transactions[i].transaction.conv::<UserTransaction>;

      let userTransaction = &transactions[i].transaction;


      println!("{:?} script - {:?}", i, userTransaction);
    }
  }

  println!("======================================================================");

  Ok(t)
}

/*
If we have something like:
 TowerStateResourceView=
    previous_proof_hash: BytesView("b352ef60364a8b2258ad06be9f27319ed4827e29dc2e6f1385d35f7fcd49cdd4"),
    verified_tower_height: 212,


-> then we need to create proof_212.json

Content:
{"height":212,
"elapsed_secs":2600,
"preimage":"2e3276590f......235aead6dde452a6",
"proof":"001ad9b93f.....6301181",
"difficulty":120000000,
"security":512
}


===================================================

for (const transaction of transactionsRes.data.result) {
          if (
            transaction.vm_status.type !== 'executed' ||
            get(transaction, 'transaction.script.function_name') !==
              'minerstate_commit'
          )
            continue

          const { bytes } = transaction
          const preimage = bytes.substring(
            runningHeight === 0 ? 168 : 164,
            runningHeight === 0 ? 2216 : 228
          )
          const proof = bytes.substring(
            runningHeight === 0 ? 2224 : 236,
            runningHeight === 0 ? 4996 : 3008
          )
          const height = runningHeight++
          const proofJson = {
            height,
            preimage,
            proof,
            elapsed_secs: 2000,
            difficulty: 120000000,
            security: 512,
          }
          archive.append(JSON.stringify(proofJson), {
            name: `proof_${height}.json`,
          })
        }

*/








#[tauri::command]
pub fn set_env(env: String) -> Result<String, CarpeError> {
  dbg!(&env);
  match env.as_ref() {
    "test" => env::set_var("NODE_ENV", "test"),
    "prod" => env::set_var("NODE_ENV", "prod"),
    _ => {},

  }

  let v = env::var("NODE_ENV").map_err(|_| { CarpeError::misc("could not get node_env") })?;
  dbg!(&v);
  Ok(v)
  
}

#[tauri::command]
pub fn get_env() -> Result<String, CarpeError> {
  let v = env::var("NODE_ENV").map_err(|_| { CarpeError::misc("could not get node_env") })?;
  dbg!(&v);
  Ok(v)
}

