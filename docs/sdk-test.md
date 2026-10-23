# SDK测试文档
需要编写单元测试，验证以下特性工作正常：
- 对于同一个应用，同一个钱包，应用看到的subject不变。
  - 当应用要求揭露的key-value字段不同时subject保持不变
  - 对于同一个秘密x，issuer多次不同签名，且签名中claims不同时保持subject不变 
- 对于不同的应用，subject不同
- 对于错误私钥签名的holder，返回签名错误
- 对于为被签名的claims，返回claims未通过验证
