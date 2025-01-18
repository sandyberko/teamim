let teamim = [
    {name: "etnahta",       char: "\u{0591}"},    
    {name: "segol",         char: "\u{0592}"},
    {name: "shalshelet",    char: "\u{0593}"},
    {name: "zaquef_qatan",  char: "\u{0594}"},
    {name: "zaquef_gadol",  char: "\u{0595}"},
    {name: "tipeha",        char: "\u{0596}"},
    {name: "revia",         char: "\u{0597}"},
    {name: "zarqa",         char: "\u{0598}"},
    {name: "pashta",        char: "\u{0599}"},
    {name: "yetiv",         char: "\u{059A}"},
    {name: "tevir",         char: "\u{059B}"},
    {name: "geresh",        char: "\u{059C}"},
    {name: "geresh_muqdam", char: "\u{059D}"},
    {name: "gershayim",     char: "\u{059E}"},
    {name: "qarney_para",   char: "\u{059F}"},
    {name: "telisha_gedola",char: "\u{05A0}"},
    {name: "pazer",         char: "\u{05A1}"},
    # ---
    {name: "munah",         char: "\u{05A3}"},
    {name: "mahapakh",      char: "\u{05A4}"},
    {name: "merkha",        char: "\u{05A5}"},
    {name: "merkha_kefula", char: "\u{05A6}"},
    {name: "darga",         char: "\u{05A7}"},
    {name: "qadma",         char: "\u{05A8}"},
    {name: "telisha_ketana",char: "\u{05A9}"},
    {name: "yerah_ben_yomo",char: "\u{05AA}"},
    {name: "oleh",          char: "\u{05AB}"},
    {name: "iluy",          char: "\u{05AC}"},
    {name: "dehi",          char: "\u{05AD}"},
    # {name: "tzinor",        char: "\u{05AE}"},
    # ---
    {name: "meteg",         char: "\u{05BD}"},
    {name: "maqaf",         char: "\u{05BE}"},
    # ---
    {name: "sof_pasuq",     char: "\u{05C3}"},
]
# Requires ffmpeg version older than 6. new versions render te'amim all wrong.
for taam in $teamim {
    (ffmpeg
        -y
        -f lavfi                                                                                                                                    
        -i $"color=color=white
                :s=64x64,format=rgba,drawtext=text='($taam.char)'
                :font='Guttman Stam'
                :ft_load_flags=0
                :fontcolor=black
                :fontsize=64
                :x=10
                :y=10"
        -frames:v 1
        -update 1
        $"assets/glyphs/($taam.name).tif"
    )
}