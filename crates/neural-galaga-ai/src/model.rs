//! Small MLP actor-critic over the structured observation vector.

use burn::nn::{Linear, LinearConfig, Relu};
use burn::prelude::*;
use burn::tensor::activation::log_softmax;

use crate::NUM_ACTIONS;
use crate::env::STACKED_OBS_SIZE;

const HIDDEN1: usize = 768;
const HIDDEN2: usize = 768;
const HIDDEN3: usize = 384;
const TRUNK_DIM: usize = 256;

#[derive(Module, Debug)]
pub struct CheatsActorCritic<B: Backend> {
    fc1: Linear<B>,
    fc2: Linear<B>,
    fc3: Linear<B>,
    relu: Relu,
    trunk: Linear<B>,
    policy_head: Linear<B>,
    value_head: Linear<B>,
}

#[derive(Config, Debug)]
pub struct CheatsActorCriticConfig;

impl CheatsActorCriticConfig {
    pub fn init<B: Backend>(&self, device: &B::Device) -> CheatsActorCritic<B> {
        CheatsActorCritic {
            fc1: LinearConfig::new(STACKED_OBS_SIZE, HIDDEN1).init(device),
            fc2: LinearConfig::new(HIDDEN1, HIDDEN2).init(device),
            fc3: LinearConfig::new(HIDDEN2, HIDDEN3).init(device),
            relu: Relu::new(),
            trunk: LinearConfig::new(HIDDEN3, TRUNK_DIM).init(device),
            policy_head: LinearConfig::new(TRUNK_DIM, NUM_ACTIONS).init(device),
            value_head: LinearConfig::new(TRUNK_DIM, 1).init(device),
        }
    }
}

pub struct CheatsOutput<B: Backend> {
    pub log_probs: Tensor<B, 2>,
    pub value: Tensor<B, 2>,
}

impl<B: Backend> CheatsActorCritic<B> {
    /// `obs` is `[batch, OBS_SIZE]`. Returns the policy / value.
    pub fn forward(&self, obs: Tensor<B, 2>) -> CheatsOutput<B> {
        let x = self.relu.forward(self.fc1.forward(obs));
        let x = self.relu.forward(self.fc2.forward(x));
        let x = self.relu.forward(self.fc3.forward(x));
        let trunk = self.relu.forward(self.trunk.forward(x));
        let logits = self.policy_head.forward(trunk.clone());
        let value = self.value_head.forward(trunk);
        CheatsOutput {
            log_probs: log_softmax(logits, 1),
            value,
        }
    }

    /// Sample one action per env via multinomial sampling.
    pub fn sample_actions(&self, obs: Tensor<B, 2>) -> (Vec<usize>, CheatsOutput<B>) {
        let output = self.forward(obs);
        let probs_data: Vec<f32> = output.log_probs.clone().exp().to_data().to_vec().unwrap();
        let batch_size = probs_data.len() / NUM_ACTIONS;

        let mut actions = Vec::with_capacity(batch_size);
        for b in 0..batch_size {
            let offset = b * NUM_ACTIONS;
            let slice = &probs_data[offset..offset + NUM_ACTIONS];
            let r: f32 = rand::random();
            let mut cumsum = 0.0;
            let mut chosen = NUM_ACTIONS - 1;
            for (i, &p) in slice.iter().enumerate() {
                cumsum += p;
                if r < cumsum {
                    chosen = i;
                    break;
                }
            }
            actions.push(chosen);
        }
        (actions, output)
    }
}
