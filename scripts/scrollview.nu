let tess_dir = "C:\\Program Files\\Tesseract-OCR"
$env.Path ++= [$tess_dir]
$env.SCROLLVIEW_PATH = $tess_dir + "\\tessdata"

def "main" [format: string] {
    print ("generating " + $format)
    (tesseract
        # --tessdata-dir './assets/tessdata/'
        # -l stam
        -l heb
        --psm 6
        ./tests/genesis1.jpg
        ./tests/genesis1_out
        ($tess_dir + "\\tessdata\\configs\\" + $format)
    )
}