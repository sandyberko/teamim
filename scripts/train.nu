#! /usr/bin/env nu
def main [
    --cont: string
    -v
] {
    if $cont != null {
        print $"(ansi blue)Continue training from ($cont)(ansi reset)"
    } else {
        print $"(ansi blue)Start training...(ansi reset)"
    }
    if $v {
        print $"(ansi yellow)Debug mode enabled(ansi reset)"
    }

    (lstmtraining
        --old_traineddata assets/langdata/heb/heb.traineddata
        --traineddata assets/langdata/stam/stam.traineddata
        --train_listfile assets/training/train.txt
        --eval_listfile assets/training/eval.txt
        --model_output assets/training/model/stam
        
        --debug_interval (if $v { -1 } else { 0 })
        
        --continue_from if $cont != null { --continue_from $cont } else { assets/langdata/heb/heb.lstm }
    )
}