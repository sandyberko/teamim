# Requires ffmpeg version older than 6. new versions render te'amim all wrong.
(ffmpeg
    -y
    -f lavfi                                                                                                                                    
    -i "color=color=white
            :s=9x16,format=rgba,drawtext=text='֓'
            :font=Arial
            :text_shaping=1
            :ft_load_flags=0
            :fontcolor=black
            :fontsize=64
            :x=0
            :y=0"
    -frames:v 1
    -update 1
    "assets/glyphs/shalshelet.tif"
)