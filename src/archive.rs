//! Workspace archive checkpoints. Git execution stays in an owned bounded worker;
//! durable phase receipts are written by the sole persistence writer.
use crate::git::CheckoutIdentity;
use serde::{Deserialize, Serialize};
use std::{sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc}, thread};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub request: String,
    pub workspace: String,
    pub checkout: CheckoutIdentity,
    pub phase: Phase,
    pub before: Option<String>,
    pub branch: Option<String>,
    pub tree: Option<String>,
    pub index_tree: Option<String>,
    pub index_hash: Option<String>,
    pub saved: Option<String>,
    pub reference: String,
    pub excluded: Vec<Vec<u8>>,
    pub message: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase { Waiting, Prepared, Saved, Archived, Failed, Cancelled }
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct View { pub records: Vec<Record>, pub pending: bool, pub error: Option<String> }
impl Record {
    pub fn new(workspace: &crate::registry::Workspace, request: &str) -> Result<Self,String> {
        uuid::Uuid::parse_str(request).map_err(|_| "Archive request must be a UUID")?;
        uuid::Uuid::parse_str(&workspace.id).map_err(|_| "Archive workspace identity unavailable")?;
        Ok(Self { request:request.into(), workspace:workspace.id.clone(), checkout:workspace.checkout.clone().ok_or("Canonical checkout unavailable")?, phase:Phase::Waiting, before:None, branch:None, tree:None, index_tree:None,index_hash:None,saved:None,reference:format!("refs/rat/archive/{}/{}",workspace.id,request),excluded:Vec::new(),message:"Stopping owned coding/setup execution and settling capture before save".into() })
    }
    pub fn blocks(&self) -> bool { !matches!(self.phase,Phase::Archived|Phase::Failed|Phase::Cancelled) }
}
pub struct Job {cancel:Arc<AtomicBool>,thread:thread::JoinHandle<()>,pub result:mpsc::Receiver<Result<Record,String>>}
impl Job {
    pub fn start(record:Record)->std::io::Result<Self>{let cancel=Arc::new(AtomicBool::new(false));let worker_cancel=cancel.clone();let(tx,result)=mpsc::sync_channel(1);let thread=thread::Builder::new().name("archive-checkpoint".into()).spawn(move||{let outcome=match record.phase{Phase::Waiting=>crate::archive_git::prepare(record,&worker_cancel),Phase::Prepared|Phase::Saved=>crate::archive_git::save(record,&worker_cancel),_=>Err("Archive phase cannot execute Git".into())};let _=tx.send(outcome);})?;Ok(Self{cancel,thread,result})}
    pub fn finished(&self)->bool{self.thread.is_finished()}
    pub fn cancel(&self){self.cancel.store(true,Ordering::Release)}
}
impl Drop for Job{fn drop(&mut self){self.cancel()}}

pub struct Coordinator {
    pub view:View, loaded:bool, current:Option<Record>, saving:Option<(uuid::Uuid,Record)>, job:Option<Job>, armed:bool,
}
impl Coordinator {
    pub fn new(storage:&crate::persistence::Worker)->Self{
        let error=storage.commands.try_send(crate::persistence::Command::ArchiveLoad).err().map(|e|e.to_string());
        Self{view:View{error,..View::default()},loaded:false,current:None,saving:None,job:None,armed:false}
    }
    pub fn ready(&self)->bool{self.loaded}
    pub fn busy(&self)->bool{self.job.is_some()||self.saving.is_some()||self.armed}
    pub fn blocks(&self,checkout:&CheckoutIdentity)->bool{
        self.current.as_ref().is_some_and(|r| (self.busy()||r.blocks())&&r.checkout.common_dir==checkout.common_dir&&r.checkout.git_dir==checkout.git_dir)
        ||self.view.records.iter().any(|r|r.blocks()&&r.checkout.common_dir==checkout.common_dir&&r.checkout.git_dir==checkout.git_dir)
    }
    pub fn begin(&mut self,workspace:&crate::registry::Workspace,request:&str,storage:&crate::persistence::Worker){
        if !self.loaded||self.busy(){self.view.error=Some("Archive receipt loading or operation pending; wait or cancel".into());return}
        if self.view.records.iter().any(|r|r.workspace==workspace.id&&r.request!=request&&r.blocks()) {self.view.error=Some("An earlier archive request is unresolved; retry/cancel its receipt before creating another".into());return}
        let existing=self.view.records.iter().find(|r|r.request==request);
        if existing.is_some_and(|r|r.workspace!=workspace.id){self.view.error=Some("Archive request already belongs to another workspace".into());return}
        let record=existing.cloned().map(Ok).unwrap_or_else(||Record::new(workspace,request));
        match record {Ok(mut r)=>{
            if matches!(r.phase,Phase::Archived){self.current=Some(r);return}
            // Failed command replies retain the prepared snapshot so same-request
            // recovery can inspect an already-created commit, never blindly retry it.
            if matches!(r.phase,Phase::Failed|Phase::Cancelled){r.phase=if r.tree.is_some(){Phase::Prepared}else{Phase::Waiting}}
            self.armed=true;self.current=Some(r.clone());self.view.error=None;self.persist(r,storage);
        },Err(e)=>self.view.error=Some(e)}
    }
    fn persist(&mut self,mut record:Record,storage:&crate::persistence::Worker){
        if record.phase==Phase::Saved {record.phase=Phase::Archived;}
        let id=uuid::Uuid::new_v4();
        match storage.commands.try_send(crate::persistence::Command::ArchiveSave(id,Box::new(record.clone()))){Ok(())=>self.saving=Some((id,record)),Err(e)=>{self.armed=false;self.view.error=Some(format!("Archive phase not durable: {e}; retry same request"));self.current=Some(record)}}
    }
    pub fn complete(&mut self,event:&crate::persistence::Completion)->Option<crate::registry::Registry>{
        match event {
            crate::persistence::Completion::Archives(result)=>{self.loaded=true;match result{Ok(records)=>self.view.records=records.clone(),Err(e)=>{self.loaded=false;self.view.error=Some(e.clone())}}}
            crate::persistence::Completion::ArchiveSaved(id,result) if self.saving.as_ref().is_some_and(|(pending,_)|pending==id)=>{
                let(_,mut record)=self.saving.take().unwrap();
                match result{Ok(registry)=>{
                    if record.phase==Phase::Saved{record.phase=Phase::Archived}
                    self.view.records.retain(|r|r.request!=record.request);self.view.records.push(record.clone());
                    self.current=Some(record.clone());if record.phase==Phase::Archived{self.armed=false}
                    return Some(registry.clone())
                },Err(e)=>{self.current=Some(record);self.armed=false;self.view.error=Some(format!("Archive metadata not saved: {e}; workspace unarchived, retry same request"))}}
            }
            _=>{}
        }None
    }
    pub fn advance(&mut self,settled:bool,storage:&crate::persistence::Worker){
        if self.job.as_ref().is_some_and(Job::finished){
            let job=self.job.take().unwrap();
            match job.result.try_recv(){
                Ok(Ok(r))=>{self.current=Some(r.clone());self.persist(r,storage)},
                result=>{let mut r=self.current.clone().unwrap();r.phase=if self.armed{Phase::Failed}else{Phase::Cancelled};r.message=match result{Ok(Err(e))=>e,_=>"Archive worker ended without a receipt; retry same request to reconcile".into()};self.view.error=Some(r.message.clone());self.armed=false;self.persist(r,storage)}
            }
        }
        if self.armed&&settled&&self.job.is_none()&&self.saving.is_none()&&let Some(r)=self.current.clone(){
            match Job::start(r){Ok(job)=>self.job=Some(job),Err(e)=>{self.armed=false;self.view.error=Some(e.to_string())}}
        }
        if !self.armed&&self.job.is_none()&&self.saving.is_none()&&let Some(mut r)=self.current.clone()&&r.blocks(){
            r.phase=Phase::Cancelled;r.message="Archive cancelled; execution remains stopped. Retry same request reconciles any partial save".into();self.current=Some(r.clone());self.persist(r,storage);
        }
        self.view.pending=self.busy();
    }
    pub fn target(&self)->Option<&CheckoutIdentity>{self.current.as_ref().filter(|_|self.armed).map(|r|&r.checkout)}
    pub fn cancel(&mut self){self.armed=false;if let Some(job)=&self.job{job.cancel()} self.view.error=Some("Archive cancelled; Stop remains in effect. A partial checkpoint/ref may exist locally; retry same request reconciles it".into());}
}
