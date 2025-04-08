# 1..=245 | each {
#     let n = $in | fill -a right -c '0' -w 3
#     wget -O assets\images\simania\($n).jpg http://sparks.simania.co.il/tora/($in).jpg
# }
[
    ['bereshit', 1..=55],
    
]