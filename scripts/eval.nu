def main [$checkpoint: string, -q] {
    (lstmeval
        --traineddata assets/langdata/stam/stam.traineddata
        --eval_listfile assets/training/eval.txt
        --model $checkpoint
        --verbosity (if $q { 0 } else { 1 })
    )
}