using System;
using TMPro;
using UnityEngine;
using Random = UnityEngine.Random;

public class Test : MonoBehaviour
{
    [SerializeField] private TMP_Text text;
    private int i32 = 0;
    private long i64 = 0;
    private float f32 = 0f;
    private double f64 = 0d;
    private string str = "test";

    private string test_str = "tst_123";
    
    private void Awake()
    {
        UpdateText();
    }

    void Update()
    {
        if (Input.GetKeyDown(KeyCode.Space))
        {
            i32 += 10;
            i64 += 15;
            f32 += 10.5f;
            f64 += 15.5d + Random.value;
            str = "test" + i32;
            UpdateText();
        }

        if (Input.GetKeyDown(KeyCode.Mouse0))
        {
            UpdateText();
        }
    }


    public void UpdateText()
    {
        text.text = $"i32: {i32}\ni64: {i64}\nf32: {f32}\nf64: {f64}\nstr: {str}";
    }

}
