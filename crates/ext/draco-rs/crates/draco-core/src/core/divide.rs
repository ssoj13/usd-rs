//! Divide by multiply utilities.
//! Reference: `_ref/draco/src/draco/core/divide.h` + `.cc`.

#[derive(Clone, Copy, Debug)]
pub struct FastDivElem {
    pub mult: u32,
    pub shift: u32,
}

pub const VP10_FASTDIV_TAB: [FastDivElem; 256] = [
    FastDivElem { mult: 0, shift: 0 },
    FastDivElem { mult: 0, shift: 0 },
    FastDivElem { mult: 0, shift: 1 },
    FastDivElem {
        mult: 1431655766,
        shift: 2,
    },
    FastDivElem { mult: 0, shift: 2 },
    FastDivElem {
        mult: 2576980378,
        shift: 3,
    },
    FastDivElem {
        mult: 1431655766,
        shift: 3,
    },
    FastDivElem {
        mult: 613566757,
        shift: 3,
    },
    FastDivElem { mult: 0, shift: 3 },
    FastDivElem {
        mult: 3340530120,
        shift: 4,
    },
    FastDivElem {
        mult: 2576980378,
        shift: 4,
    },
    FastDivElem {
        mult: 1952257862,
        shift: 4,
    },
    FastDivElem {
        mult: 1431655766,
        shift: 4,
    },
    FastDivElem {
        mult: 991146300,
        shift: 4,
    },
    FastDivElem {
        mult: 613566757,
        shift: 4,
    },
    FastDivElem {
        mult: 286331154,
        shift: 4,
    },
    FastDivElem { mult: 0, shift: 4 },
    FastDivElem {
        mult: 3789677026,
        shift: 5,
    },
    FastDivElem {
        mult: 3340530120,
        shift: 5,
    },
    FastDivElem {
        mult: 2938661835,
        shift: 5,
    },
    FastDivElem {
        mult: 2576980378,
        shift: 5,
    },
    FastDivElem {
        mult: 2249744775,
        shift: 5,
    },
    FastDivElem {
        mult: 1952257862,
        shift: 5,
    },
    FastDivElem {
        mult: 1680639377,
        shift: 5,
    },
    FastDivElem {
        mult: 1431655766,
        shift: 5,
    },
    FastDivElem {
        mult: 1202590843,
        shift: 5,
    },
    FastDivElem {
        mult: 991146300,
        shift: 5,
    },
    FastDivElem {
        mult: 795364315,
        shift: 5,
    },
    FastDivElem {
        mult: 613566757,
        shift: 5,
    },
    FastDivElem {
        mult: 444306962,
        shift: 5,
    },
    FastDivElem {
        mult: 286331154,
        shift: 5,
    },
    FastDivElem {
        mult: 138547333,
        shift: 5,
    },
    FastDivElem { mult: 0, shift: 5 },
    FastDivElem {
        mult: 4034666248,
        shift: 6,
    },
    FastDivElem {
        mult: 3789677026,
        shift: 6,
    },
    FastDivElem {
        mult: 3558687189,
        shift: 6,
    },
    FastDivElem {
        mult: 3340530120,
        shift: 6,
    },
    FastDivElem {
        mult: 3134165325,
        shift: 6,
    },
    FastDivElem {
        mult: 2938661835,
        shift: 6,
    },
    FastDivElem {
        mult: 2753184165,
        shift: 6,
    },
    FastDivElem {
        mult: 2576980378,
        shift: 6,
    },
    FastDivElem {
        mult: 2409371898,
        shift: 6,
    },
    FastDivElem {
        mult: 2249744775,
        shift: 6,
    },
    FastDivElem {
        mult: 2097542168,
        shift: 6,
    },
    FastDivElem {
        mult: 1952257862,
        shift: 6,
    },
    FastDivElem {
        mult: 1813430637,
        shift: 6,
    },
    FastDivElem {
        mult: 1680639377,
        shift: 6,
    },
    FastDivElem {
        mult: 1553498810,
        shift: 6,
    },
    FastDivElem {
        mult: 1431655766,
        shift: 6,
    },
    FastDivElem {
        mult: 1314785907,
        shift: 6,
    },
    FastDivElem {
        mult: 1202590843,
        shift: 6,
    },
    FastDivElem {
        mult: 1094795586,
        shift: 6,
    },
    FastDivElem {
        mult: 991146300,
        shift: 6,
    },
    FastDivElem {
        mult: 891408307,
        shift: 6,
    },
    FastDivElem {
        mult: 795364315,
        shift: 6,
    },
    FastDivElem {
        mult: 702812831,
        shift: 6,
    },
    FastDivElem {
        mult: 613566757,
        shift: 6,
    },
    FastDivElem {
        mult: 527452125,
        shift: 6,
    },
    FastDivElem {
        mult: 444306962,
        shift: 6,
    },
    FastDivElem {
        mult: 363980280,
        shift: 6,
    },
    FastDivElem {
        mult: 286331154,
        shift: 6,
    },
    FastDivElem {
        mult: 211227900,
        shift: 6,
    },
    FastDivElem {
        mult: 138547333,
        shift: 6,
    },
    FastDivElem {
        mult: 68174085,
        shift: 6,
    },
    FastDivElem { mult: 0, shift: 6 },
    FastDivElem {
        mult: 4162814457,
        shift: 7,
    },
    FastDivElem {
        mult: 4034666248,
        shift: 7,
    },
    FastDivElem {
        mult: 3910343360,
        shift: 7,
    },
    FastDivElem {
        mult: 3789677026,
        shift: 7,
    },
    FastDivElem {
        mult: 3672508268,
        shift: 7,
    },
    FastDivElem {
        mult: 3558687189,
        shift: 7,
    },
    FastDivElem {
        mult: 3448072337,
        shift: 7,
    },
    FastDivElem {
        mult: 3340530120,
        shift: 7,
    },
    FastDivElem {
        mult: 3235934265,
        shift: 7,
    },
    FastDivElem {
        mult: 3134165325,
        shift: 7,
    },
    FastDivElem {
        mult: 3035110223,
        shift: 7,
    },
    FastDivElem {
        mult: 2938661835,
        shift: 7,
    },
    FastDivElem {
        mult: 2844718599,
        shift: 7,
    },
    FastDivElem {
        mult: 2753184165,
        shift: 7,
    },
    FastDivElem {
        mult: 2663967058,
        shift: 7,
    },
    FastDivElem {
        mult: 2576980378,
        shift: 7,
    },
    FastDivElem {
        mult: 2492141518,
        shift: 7,
    },
    FastDivElem {
        mult: 2409371898,
        shift: 7,
    },
    FastDivElem {
        mult: 2328596727,
        shift: 7,
    },
    FastDivElem {
        mult: 2249744775,
        shift: 7,
    },
    FastDivElem {
        mult: 2172748162,
        shift: 7,
    },
    FastDivElem {
        mult: 2097542168,
        shift: 7,
    },
    FastDivElem {
        mult: 2024065048,
        shift: 7,
    },
    FastDivElem {
        mult: 1952257862,
        shift: 7,
    },
    FastDivElem {
        mult: 1882064321,
        shift: 7,
    },
    FastDivElem {
        mult: 1813430637,
        shift: 7,
    },
    FastDivElem {
        mult: 1746305385,
        shift: 7,
    },
    FastDivElem {
        mult: 1680639377,
        shift: 7,
    },
    FastDivElem {
        mult: 1616385542,
        shift: 7,
    },
    FastDivElem {
        mult: 1553498810,
        shift: 7,
    },
    FastDivElem {
        mult: 1491936009,
        shift: 7,
    },
    FastDivElem {
        mult: 1431655766,
        shift: 7,
    },
    FastDivElem {
        mult: 1372618415,
        shift: 7,
    },
    FastDivElem {
        mult: 1314785907,
        shift: 7,
    },
    FastDivElem {
        mult: 1258121734,
        shift: 7,
    },
    FastDivElem {
        mult: 1202590843,
        shift: 7,
    },
    FastDivElem {
        mult: 1148159575,
        shift: 7,
    },
    FastDivElem {
        mult: 1094795586,
        shift: 7,
    },
    FastDivElem {
        mult: 1042467791,
        shift: 7,
    },
    FastDivElem {
        mult: 991146300,
        shift: 7,
    },
    FastDivElem {
        mult: 940802361,
        shift: 7,
    },
    FastDivElem {
        mult: 891408307,
        shift: 7,
    },
    FastDivElem {
        mult: 842937507,
        shift: 7,
    },
    FastDivElem {
        mult: 795364315,
        shift: 7,
    },
    FastDivElem {
        mult: 748664025,
        shift: 7,
    },
    FastDivElem {
        mult: 702812831,
        shift: 7,
    },
    FastDivElem {
        mult: 657787785,
        shift: 7,
    },
    FastDivElem {
        mult: 613566757,
        shift: 7,
    },
    FastDivElem {
        mult: 570128403,
        shift: 7,
    },
    FastDivElem {
        mult: 527452125,
        shift: 7,
    },
    FastDivElem {
        mult: 485518043,
        shift: 7,
    },
    FastDivElem {
        mult: 444306962,
        shift: 7,
    },
    FastDivElem {
        mult: 403800345,
        shift: 7,
    },
    FastDivElem {
        mult: 363980280,
        shift: 7,
    },
    FastDivElem {
        mult: 324829460,
        shift: 7,
    },
    FastDivElem {
        mult: 286331154,
        shift: 7,
    },
    FastDivElem {
        mult: 248469183,
        shift: 7,
    },
    FastDivElem {
        mult: 211227900,
        shift: 7,
    },
    FastDivElem {
        mult: 174592167,
        shift: 7,
    },
    FastDivElem {
        mult: 138547333,
        shift: 7,
    },
    FastDivElem {
        mult: 103079216,
        shift: 7,
    },
    FastDivElem {
        mult: 68174085,
        shift: 7,
    },
    FastDivElem {
        mult: 33818641,
        shift: 7,
    },
    FastDivElem { mult: 0, shift: 7 },
    FastDivElem {
        mult: 4228378656,
        shift: 8,
    },
    FastDivElem {
        mult: 4162814457,
        shift: 8,
    },
    FastDivElem {
        mult: 4098251237,
        shift: 8,
    },
    FastDivElem {
        mult: 4034666248,
        shift: 8,
    },
    FastDivElem {
        mult: 3972037425,
        shift: 8,
    },
    FastDivElem {
        mult: 3910343360,
        shift: 8,
    },
    FastDivElem {
        mult: 3849563281,
        shift: 8,
    },
    FastDivElem {
        mult: 3789677026,
        shift: 8,
    },
    FastDivElem {
        mult: 3730665024,
        shift: 8,
    },
    FastDivElem {
        mult: 3672508268,
        shift: 8,
    },
    FastDivElem {
        mult: 3615188300,
        shift: 8,
    },
    FastDivElem {
        mult: 3558687189,
        shift: 8,
    },
    FastDivElem {
        mult: 3502987511,
        shift: 8,
    },
    FastDivElem {
        mult: 3448072337,
        shift: 8,
    },
    FastDivElem {
        mult: 3393925206,
        shift: 8,
    },
    FastDivElem {
        mult: 3340530120,
        shift: 8,
    },
    FastDivElem {
        mult: 3287871517,
        shift: 8,
    },
    FastDivElem {
        mult: 3235934265,
        shift: 8,
    },
    FastDivElem {
        mult: 3184703642,
        shift: 8,
    },
    FastDivElem {
        mult: 3134165325,
        shift: 8,
    },
    FastDivElem {
        mult: 3084305374,
        shift: 8,
    },
    FastDivElem {
        mult: 3035110223,
        shift: 8,
    },
    FastDivElem {
        mult: 2986566663,
        shift: 8,
    },
    FastDivElem {
        mult: 2938661835,
        shift: 8,
    },
    FastDivElem {
        mult: 2891383213,
        shift: 8,
    },
    FastDivElem {
        mult: 2844718599,
        shift: 8,
    },
    FastDivElem {
        mult: 2798656110,
        shift: 8,
    },
    FastDivElem {
        mult: 2753184165,
        shift: 8,
    },
    FastDivElem {
        mult: 2708291480,
        shift: 8,
    },
    FastDivElem {
        mult: 2663967058,
        shift: 8,
    },
    FastDivElem {
        mult: 2620200175,
        shift: 8,
    },
    FastDivElem {
        mult: 2576980378,
        shift: 8,
    },
    FastDivElem {
        mult: 2534297473,
        shift: 8,
    },
    FastDivElem {
        mult: 2492141518,
        shift: 8,
    },
    FastDivElem {
        mult: 2450502814,
        shift: 8,
    },
    FastDivElem {
        mult: 2409371898,
        shift: 8,
    },
    FastDivElem {
        mult: 2368739540,
        shift: 8,
    },
    FastDivElem {
        mult: 2328596727,
        shift: 8,
    },
    FastDivElem {
        mult: 2288934667,
        shift: 8,
    },
    FastDivElem {
        mult: 2249744775,
        shift: 8,
    },
    FastDivElem {
        mult: 2211018668,
        shift: 8,
    },
    FastDivElem {
        mult: 2172748162,
        shift: 8,
    },
    FastDivElem {
        mult: 2134925265,
        shift: 8,
    },
    FastDivElem {
        mult: 2097542168,
        shift: 8,
    },
    FastDivElem {
        mult: 2060591247,
        shift: 8,
    },
    FastDivElem {
        mult: 2024065048,
        shift: 8,
    },
    FastDivElem {
        mult: 1987956292,
        shift: 8,
    },
    FastDivElem {
        mult: 1952257862,
        shift: 8,
    },
    FastDivElem {
        mult: 1916962805,
        shift: 8,
    },
    FastDivElem {
        mult: 1882064321,
        shift: 8,
    },
    FastDivElem {
        mult: 1847555765,
        shift: 8,
    },
    FastDivElem {
        mult: 1813430637,
        shift: 8,
    },
    FastDivElem {
        mult: 1779682582,
        shift: 8,
    },
    FastDivElem {
        mult: 1746305385,
        shift: 8,
    },
    FastDivElem {
        mult: 1713292966,
        shift: 8,
    },
    FastDivElem {
        mult: 1680639377,
        shift: 8,
    },
    FastDivElem {
        mult: 1648338801,
        shift: 8,
    },
    FastDivElem {
        mult: 1616385542,
        shift: 8,
    },
    FastDivElem {
        mult: 1584774030,
        shift: 8,
    },
    FastDivElem {
        mult: 1553498810,
        shift: 8,
    },
    FastDivElem {
        mult: 1522554545,
        shift: 8,
    },
    FastDivElem {
        mult: 1491936009,
        shift: 8,
    },
    FastDivElem {
        mult: 1461638086,
        shift: 8,
    },
    FastDivElem {
        mult: 1431655766,
        shift: 8,
    },
    FastDivElem {
        mult: 1401984144,
        shift: 8,
    },
    FastDivElem {
        mult: 1372618415,
        shift: 8,
    },
    FastDivElem {
        mult: 1343553873,
        shift: 8,
    },
    FastDivElem {
        mult: 1314785907,
        shift: 8,
    },
    FastDivElem {
        mult: 1286310003,
        shift: 8,
    },
    FastDivElem {
        mult: 1258121734,
        shift: 8,
    },
    FastDivElem {
        mult: 1230216764,
        shift: 8,
    },
    FastDivElem {
        mult: 1202590843,
        shift: 8,
    },
    FastDivElem {
        mult: 1175239808,
        shift: 8,
    },
    FastDivElem {
        mult: 1148159575,
        shift: 8,
    },
    FastDivElem {
        mult: 1121346142,
        shift: 8,
    },
    FastDivElem {
        mult: 1094795586,
        shift: 8,
    },
    FastDivElem {
        mult: 1068504060,
        shift: 8,
    },
    FastDivElem {
        mult: 1042467791,
        shift: 8,
    },
    FastDivElem {
        mult: 1016683080,
        shift: 8,
    },
    FastDivElem {
        mult: 991146300,
        shift: 8,
    },
    FastDivElem {
        mult: 965853890,
        shift: 8,
    },
    FastDivElem {
        mult: 940802361,
        shift: 8,
    },
    FastDivElem {
        mult: 915988286,
        shift: 8,
    },
    FastDivElem {
        mult: 891408307,
        shift: 8,
    },
    FastDivElem {
        mult: 867059126,
        shift: 8,
    },
    FastDivElem {
        mult: 842937507,
        shift: 8,
    },
    FastDivElem {
        mult: 819040276,
        shift: 8,
    },
    FastDivElem {
        mult: 795364315,
        shift: 8,
    },
    FastDivElem {
        mult: 771906565,
        shift: 8,
    },
    FastDivElem {
        mult: 748664025,
        shift: 8,
    },
    FastDivElem {
        mult: 725633745,
        shift: 8,
    },
    FastDivElem {
        mult: 702812831,
        shift: 8,
    },
    FastDivElem {
        mult: 680198441,
        shift: 8,
    },
    FastDivElem {
        mult: 657787785,
        shift: 8,
    },
    FastDivElem {
        mult: 635578121,
        shift: 8,
    },
    FastDivElem {
        mult: 613566757,
        shift: 8,
    },
    FastDivElem {
        mult: 591751050,
        shift: 8,
    },
    FastDivElem {
        mult: 570128403,
        shift: 8,
    },
    FastDivElem {
        mult: 548696263,
        shift: 8,
    },
    FastDivElem {
        mult: 527452125,
        shift: 8,
    },
    FastDivElem {
        mult: 506393524,
        shift: 8,
    },
    FastDivElem {
        mult: 485518043,
        shift: 8,
    },
    FastDivElem {
        mult: 464823301,
        shift: 8,
    },
    FastDivElem {
        mult: 444306962,
        shift: 8,
    },
    FastDivElem {
        mult: 423966729,
        shift: 8,
    },
    FastDivElem {
        mult: 403800345,
        shift: 8,
    },
    FastDivElem {
        mult: 383805589,
        shift: 8,
    },
    FastDivElem {
        mult: 363980280,
        shift: 8,
    },
    FastDivElem {
        mult: 344322273,
        shift: 8,
    },
    FastDivElem {
        mult: 324829460,
        shift: 8,
    },
    FastDivElem {
        mult: 305499766,
        shift: 8,
    },
    FastDivElem {
        mult: 286331154,
        shift: 8,
    },
    FastDivElem {
        mult: 267321616,
        shift: 8,
    },
    FastDivElem {
        mult: 248469183,
        shift: 8,
    },
    FastDivElem {
        mult: 229771913,
        shift: 8,
    },
    FastDivElem {
        mult: 211227900,
        shift: 8,
    },
    FastDivElem {
        mult: 192835267,
        shift: 8,
    },
    FastDivElem {
        mult: 174592167,
        shift: 8,
    },
    FastDivElem {
        mult: 156496785,
        shift: 8,
    },
    FastDivElem {
        mult: 138547333,
        shift: 8,
    },
    FastDivElem {
        mult: 120742053,
        shift: 8,
    },
    FastDivElem {
        mult: 103079216,
        shift: 8,
    },
    FastDivElem {
        mult: 85557118,
        shift: 8,
    },
    FastDivElem {
        mult: 68174085,
        shift: 8,
    },
    FastDivElem {
        mult: 50928466,
        shift: 8,
    },
    FastDivElem {
        mult: 33818641,
        shift: 8,
    },
    FastDivElem {
        mult: 16843010,
        shift: 8,
    },
];

#[inline]
pub fn fastdiv(x: u32, y: usize) -> u32 {
    let t = ((x as u64) * (VP10_FASTDIV_TAB[y].mult as u64)) >> (u32::BITS);
    ((t as u32) + x) >> VP10_FASTDIV_TAB[y].shift
}
